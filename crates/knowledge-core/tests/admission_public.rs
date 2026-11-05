#![cfg(unix)]

//! Independent public-interface admission tests derived only from the accepted owner
//! documents: `crates/knowledge-core/README.md`, `docs/specs/storage.md`
//! ("Effective Resource Accounting"), and rendered public Rustdoc for
//! `knowledge_core::admission`, `knowledge_core::jobs`, and `knowledge_core::backend::Budget`.
//! No implementation source was consulted. A `Holding`/`Claim` `Drop` is asserted to release
//! nothing, and every availability assertion is made from the publicly exposed status, never
//! from a guessed OS memory figure.

use std::fs::{DirBuilder, File};
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use knowledge_core::admission::{
    self, Account, Configuration, Decision, Error, Origin, Scheduling, Waiting,
};
use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{
    JobError, Key, Limits, Outcome, Priority, Queue, Reservation, State, Submission,
};
use sha2::{Digest, Sha256};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// Explicit, deterministic effective ceilings for the shared process owner.
const WHOLE_MEMORY: u64 = 16 * MIB;
const WHOLE_STORAGE: u64 = MIB;

static SERIAL: Mutex<()> = Mutex::new(());
static ACCOUNT: OnceLock<&'static Account> = OnceLock::new();
static PRIMARY_DIR: OnceLock<PathBuf> = OnceLock::new();
static DIR_SEQ: AtomicU64 = AtomicU64::new(0);

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn unique_private_dir(tag: &str) -> PathBuf {
    let seq = DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "omni-admission-public-{}-{nanos}-{seq}-{tag}",
        std::process::id()
    ));
    DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .expect("create private test directory");
    path
}

fn open_private_dir(tag: &str) -> File {
    let path = unique_private_dir(tag);
    File::open(&path).expect("open admitted private directory")
}

fn open_primary_dir() -> File {
    let path = unique_private_dir("primary");
    let file = File::open(&path).expect("open admitted private directory");
    let _ = PRIMARY_DIR.set(path);
    file
}

fn scheduling() -> Scheduling {
    Scheduling {
        max_pending: 16,
        max_records: 32,
        max_descriptor_bytes: 8192,
        per_mount: 1,
    }
}

fn configuration() -> Configuration {
    Configuration {
        memory_bytes: Some(WHOLE_MEMORY),
        storage_bytes: Some(WHOLE_STORAGE),
    }
}

fn account() -> &'static Account {
    ACCOUNT.get_or_init(|| {
        Account::start(open_primary_dir(), configuration(), scheduling())
            .expect("establish the one process owner")
    })
}

fn digest(value: u64) -> ContentDigest {
    ContentDigest::new(format!("{value:064x}")).expect("checked digest spelling")
}

/// Digest of bytes this test actually owns. This is a claimed digest derived from synthetic
/// bytes, never a fabricated claim that a source was verified.
fn synthetic_digest(bytes: &[u8]) -> ContentDigest {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        hex.push_str(&format!("{byte:02x}"));
    }
    ContentDigest::new(hex).expect("synthetic SHA-256 is 64 lowercase hex characters")
}

fn budget() -> Budget {
    Budget {
        max_files: 1,
        max_bytes: 1,
        max_millis: 1,
        max_memory_bytes: 1,
    }
}

fn key<'a>(
    context: &'a str,
    library: &'a str,
    mount: &'a str,
    source: &'a str,
    version: &'a ContentDigest,
    recipe: &'a ContentDigest,
    processor: &'a ContentDigest,
) -> Key<'a> {
    Key {
        context,
        library,
        mount: Some(mount),
        source,
        version,
        recipe,
        processor,
    }
}

fn floor_for(capacity_bytes: u64) -> u64 {
    let five_percent = capacity_bytes.div_ceil(20);
    if five_percent > 2 * GIB {
        five_percent
    } else {
        2 * GIB
    }
}

fn ready_holding(account: &Account, need: Reservation) -> admission::Holding {
    match account
        .reserve(need, Origin::BeforeVerification)
        .expect("a need inside the whole ceiling and current availability is not an error")
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(_) => panic!("need was expected to fit current availability"),
    }
}

#[test]
fn start_and_current_share_one_process_owner_and_one_queue() {
    let _guard = serial();
    let owner = account();
    let reused = Account::current().expect("current must reuse the started owner");
    assert!(
        std::ptr::eq(owner, reused),
        "current() must not publish an alternate empty account"
    );
    assert!(
        std::ptr::eq(owner.queue(), reused.queue()),
        "every host must borrow the same queue, not a per-library pool"
    );
    assert!(
        std::ptr::eq(owner.queue(), owner.queue()),
        "queue() must remain the same owner between calls"
    );

    // A standalone Queue::new is scoped cooperative accounting only; it must not feed
    // the process owner's queue.
    let standalone = Queue::new(Limits {
        max_pending: 1,
        max_records: 1,
        max_descriptor_bytes: 4096,
        per_mount: 1,
        memory_bytes: 64 * KIB,
        temporary_bytes: 64 * KIB,
    })
    .expect("a consistent standalone queue");
    let version = digest(11);
    let recipe = digest(12);
    let processor = digest(13);
    let before = owner.queue().usage().expect("account usage").jobs;
    let submitted = standalone
        .submit(Submission {
            operation_id: "standalone-op",
            job_id: "standalone-job",
            key: key(
                "standalone",
                "lib",
                "m",
                "src",
                &version,
                &recipe,
                &processor,
            ),
            budget: budget(),
            reservation: Reservation {
                memory_bytes: KIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("standalone submit");
    assert!(submitted.created);
    assert_eq!(standalone.usage().expect("standalone usage").jobs, 1);
    assert_eq!(
        owner.queue().usage().expect("account usage").jobs,
        before,
        "standalone Queue::new must not be the process account"
    );
}

#[test]
fn policy_exposes_effective_startup_policy_and_native_volume_facts() {
    let _guard = serial();
    let owner = account();
    let policy = owner.policy();
    assert_eq!(
        policy.configuration.memory_bytes,
        Some(WHOLE_MEMORY),
        "caller override is exposed, not silently normalized"
    );
    assert_eq!(
        policy.configuration.storage_bytes,
        Some(WHOLE_STORAGE),
        "a further storage ceiling cannot spend the retained floor"
    );
    assert_eq!(
        policy.memory_bytes, WHOLE_MEMORY,
        "explicit memory override is the effective whole ceiling"
    );
    assert_eq!(
        policy.storage_bytes, WHOLE_STORAGE,
        "a storage override below available-minus-floor is the further ceiling"
    );
    assert!(
        policy.physical_memory_bytes > 0,
        "startup physical RAM is observed and exposed, never guessed"
    );

    let volume = policy.startup_volume;
    assert!(
        volume.capacity_bytes >= volume.available_bytes,
        "available bytes cannot exceed volume capacity"
    );
    assert_eq!(
        volume.floor_bytes,
        floor_for(volume.capacity_bytes),
        "the retained floor is max(2 GiB, ceil(capacity/20))"
    );
    assert!(
        volume.device != 0 && volume.directory != 0,
        "the selected descriptor is observed without pathname re-resolution"
    );

    let status = owner.status().expect("fresh status");
    assert_eq!(status.policy.memory_bytes, policy.memory_bytes);
    assert_eq!(status.policy.storage_bytes, policy.storage_bytes);
    assert_eq!(status.policy.configuration, policy.configuration);
    assert!(status.volume.capacity_bytes >= status.volume.available_bytes);
    assert_eq!(
        status.volume.floor_bytes,
        floor_for(status.volume.capacity_bytes)
    );
    // Memory availability is the remaining logical promise budget. It is not a
    // measured RSS reading and the test makes no physical-RAM enforcement claim.
    assert_eq!(
        status.memory_available,
        WHOLE_MEMORY - status.reserved.memory_bytes,
        "available memory is remaining promises over the effective ceiling"
    );
}

#[test]
fn second_start_is_already_started_and_cannot_reset_live_charges() {
    let _guard = serial();
    let owner = account();
    let reserved_before = owner.status().expect("status").reserved.memory_bytes;
    let holding = ready_holding(
        owner,
        Reservation {
            memory_bytes: 2 * MIB,
            temporary_bytes: 0,
        },
    );
    let charged = owner.status().expect("status").reserved.memory_bytes;
    assert_eq!(
        charged,
        reserved_before + 2 * MIB,
        "the startup cost is charged to the sole account"
    );

    let error = match Account::start(
        open_private_dir("second"),
        Configuration {
            memory_bytes: Some(0),
            storage_bytes: Some(0),
        },
        scheduling(),
    ) {
        Ok(_) => panic!("a second process owner must be refused"),
        Err(error) => error,
    };
    assert!(
        matches!(error, Error::AlreadyStarted),
        "reconfiguration cannot reset live charges"
    );
    assert!(
        std::ptr::eq(
            owner,
            Account::current().expect("the original owner survives a failed start")
        ),
        "a failed attempt publishes no usable alternate account"
    );
    assert_eq!(
        owner.policy().memory_bytes,
        WHOLE_MEMORY,
        "failed reconfiguration must not change the effective policy"
    );
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged,
        "failed reconfiguration must not release live charges"
    );

    let mut holding = holding;
    holding
        .finish()
        .expect("release after the trusted owner establishes settlement");
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        reserved_before,
        "finish releases exactly the retained promise"
    );
}

#[test]
fn need_above_the_whole_ceiling_is_a_typed_refusal_not_waiting() {
    let _guard = serial();
    let owner = account();
    let before = owner.status().expect("status");
    let error = match owner.reserve(
        Reservation {
            memory_bytes: WHOLE_MEMORY + 1,
            temporary_bytes: 0,
        },
        Origin::BeforeVerification,
    ) {
        Ok(_) => panic!("a memory need above the whole ceiling must be refused"),
        Err(error) => error,
    };
    assert!(
        matches!(error, Error::TooLargeMemory),
        "impossible whole-ceiling need is typed, not Waiting"
    );
    let error = match owner.reserve(
        Reservation {
            memory_bytes: 0,
            temporary_bytes: WHOLE_STORAGE + 1,
        },
        Origin::BeforeVerification,
    ) {
        Ok(_) => panic!("a storage need above the whole ceiling must be refused"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::TooLargeStorage));
    let after = owner.status().expect("status");
    assert_eq!(
        after.reserved.memory_bytes, before.reserved.memory_bytes,
        "a typed refusal acquires nothing"
    );
    assert_eq!(
        after.reserved.temporary_bytes,
        before.reserved.temporary_bytes
    );
    assert_eq!(after.holdings, before.holdings);

    // Account::submit refuses the same whole-budget violations before queue admission.
    let version = digest(21);
    let recipe = digest(22);
    let processor = digest(23);
    let refused = match owner.submit(Submission {
        operation_id: "op-whole-memory",
        job_id: "job-whole-memory",
        key: key("refusal", "lib", "m", "src", &version, &recipe, &processor),
        budget: budget(),
        reservation: Reservation {
            memory_bytes: WHOLE_MEMORY + 1,
            temporary_bytes: 0,
        },
        priority: Priority::Normal,
        max_attempts: 1,
    }) {
        Ok(_) => panic!("submit must refuse a claim above the whole memory ceiling"),
        Err(error) => error,
    };
    assert!(matches!(refused, Error::TooLargeMemory));
    let version = digest(24);
    let recipe = digest(25);
    let processor = digest(26);
    let refused = match owner.submit(Submission {
        operation_id: "op-whole-storage",
        job_id: "job-whole-storage",
        key: key("refusal", "lib", "m", "src", &version, &recipe, &processor),
        budget: budget(),
        reservation: Reservation {
            memory_bytes: 0,
            temporary_bytes: WHOLE_STORAGE + 1,
        },
        priority: Priority::Normal,
        max_attempts: 1,
    }) {
        Ok(_) => panic!("submit must refuse a claim above the whole storage ceiling"),
        Err(error) => error,
    };
    assert!(matches!(refused, Error::TooLargeStorage));
    assert_eq!(
        owner.queue().usage().expect("usage").pending,
        before.queue.pending,
        "refused submissions never enter the queue"
    );
}

#[test]
fn need_within_the_whole_but_above_current_availability_waits_without_partial_charge() {
    let _guard = serial();
    let owner = account();
    let before = owner.status().expect("status");

    let memory_available = before.memory_available;
    assert!(
        memory_available > 0,
        "the shared account exposes current memory availability"
    );
    let mut memory_hold = match owner.reserve(
        Reservation {
            memory_bytes: memory_available,
            temporary_bytes: 0,
        },
        Origin::Startup,
    ) {
        Ok(Decision::Ready(holding)) => holding,
        Ok(Decision::Waiting(_)) => panic!("a need equal to current availability must be admitted"),
        Err(error) => {
            panic!("inside whole ceiling and current availability must not refuse: {error:?}")
        }
    };
    let held = owner.status().expect("status");
    assert_eq!(
        held.memory_available, 0,
        "the whole current availability was reserved"
    );
    let charged = held.reserved.memory_bytes;
    let holdings = held.holdings;

    // One byte is within the whole ceiling but above zero current availability.
    let decision = owner
        .reserve(
            Reservation {
                memory_bytes: 1,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .expect("availability contention is not an error");
    match decision {
        Decision::Waiting(Waiting::Memory) => {}
        Decision::Waiting(Waiting::Storage) => {
            panic!("memory contention must be reported as Waiting(Memory)")
        }
        Decision::Ready(_) => panic!("a need above current availability must not be admitted"),
    }
    let after_wait = owner.status().expect("status");
    assert_eq!(
        after_wait.reserved.memory_bytes, charged,
        "Waiting acquires no partial memory"
    );
    assert_eq!(
        after_wait.holdings, holdings,
        "Waiting does not allocate a holding record"
    );

    // Grow checks the complete causing-job need against the whole ceiling.
    let excess = WHOLE_MEMORY - memory_available + 1;
    let grow = match memory_hold.grow(Reservation {
        memory_bytes: excess,
        temporary_bytes: 0,
    }) {
        Ok(_) => panic!("growth above the whole ceiling must not be admitted"),
        Err(error) => error,
    };
    assert!(
        matches!(grow, Error::TooLargeMemory),
        "the complete causing-job need is typed against the whole ceiling"
    );
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged,
        "a refused growth changes nothing"
    );

    // Storage contention is independently visible and distinct.
    let storage_available = owner.status().expect("status").storage_available;
    assert!(
        storage_available > 0,
        "the shared account exposes current generated-storage availability"
    );
    let mut storage_hold = match owner.reserve(
        Reservation {
            memory_bytes: 0,
            temporary_bytes: storage_available,
        },
        Origin::Startup,
    ) {
        Ok(Decision::Ready(holding)) => holding,
        Ok(Decision::Waiting(_)) => panic!("a need equal to storage availability must be admitted"),
        Err(error) => panic!("inside whole storage ceiling must not refuse: {error:?}"),
    };
    let storage_charged = owner.status().expect("status").reserved.temporary_bytes;
    assert_eq!(owner.status().expect("status").storage_available, 0);
    let decision = owner
        .reserve(
            Reservation {
                memory_bytes: 0,
                temporary_bytes: 1,
            },
            Origin::BeforeVerification,
        )
        .expect("storage availability contention is not an error");
    match decision {
        Decision::Waiting(Waiting::Storage) => {}
        Decision::Waiting(Waiting::Memory) => {
            panic!("storage contention must be reported as Waiting(Storage)")
        }
        Decision::Ready(_) => {
            panic!("a need above current storage availability must not be admitted")
        }
    }
    assert_eq!(
        owner.status().expect("status").reserved.temporary_bytes,
        storage_charged,
        "Waiting acquires no partial storage"
    );

    memory_hold.finish().expect("release after settlement");
    storage_hold.finish().expect("release after settlement");
    let settled = owner.status().expect("status");
    assert_eq!(settled.reserved.memory_bytes, before.reserved.memory_bytes);
    assert_eq!(
        settled.reserved.temporary_bytes,
        before.reserved.temporary_bytes
    );
}

#[test]
fn dropping_a_holding_releases_no_capacity() {
    let _guard = serial();
    let owner = account();
    let before = owner.status().expect("status");
    let holding = ready_holding(
        owner,
        Reservation {
            memory_bytes: KIB,
            temporary_bytes: 0,
        },
    );
    let charged = owner.status().expect("status");
    assert_eq!(
        charged.reserved.memory_bytes,
        before.reserved.memory_bytes + KIB
    );
    assert_eq!(charged.memory_available, before.memory_available - KIB);

    drop(holding);
    let after = owner.status().expect("status");
    assert_eq!(
        after.reserved.memory_bytes, charged.reserved.memory_bytes,
        "Drop releases NOTHING; a lost handle may still have live resources behind it"
    );
    assert_eq!(after.memory_available, charged.memory_available);
    assert_eq!(
        after.holdings, charged.holdings,
        "the lost capability remains a bounded retained holding"
    );
}

#[test]
fn per_mount_bound_limits_concurrent_unsettled_executions() {
    let _guard = serial();
    let owner = account();
    let queue = owner.queue();
    let before = queue.usage().expect("usage");

    let version_a = digest(41);
    let recipe_a = digest(42);
    let processor_a = digest(43);
    let submitted_a = owner
        .submit(Submission {
            operation_id: "op-bound-a",
            job_id: "job-bound-a",
            key: key(
                "bound",
                "lib",
                "mount-a",
                "src",
                &version_a,
                &recipe_a,
                &processor_a,
            ),
            budget: budget(),
            reservation: Reservation {
                memory_bytes: 64 * KIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit job A");
    assert!(submitted_a.created);
    let claim_a = queue
        .claim(Instant::now())
        .expect("claim call")
        .expect("job A is eligible");
    assert_eq!(claim_a.job(), submitted_a.job);
    let after_a = queue.usage().expect("usage");
    assert_eq!(after_a.unsettled, before.unsettled + 1);
    assert_eq!(after_a.pending, before.pending);

    let version_b = digest(44);
    let recipe_b = digest(45);
    let processor_b = digest(46);
    let submitted_b = owner
        .submit(Submission {
            operation_id: "op-bound-b",
            job_id: "job-bound-b",
            key: key(
                "bound",
                "lib",
                "mount-a",
                "src",
                &version_b,
                &recipe_b,
                &processor_b,
            ),
            budget: budget(),
            reservation: Reservation {
                memory_bytes: 64 * KIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit job B");
    assert!(submitted_b.created);
    let blocked = queue.claim(Instant::now()).expect("claim call");
    assert!(
        blocked.is_none(),
        "per_mount=1 must block a second unsettled execution on the same mount"
    );

    let settled_a = queue
        .settle(
            &claim_a,
            key(
                "bound",
                "lib",
                "mount-a",
                "src",
                &version_a,
                &recipe_a,
                &processor_a,
            ),
            Outcome::Completed,
        )
        .expect("settle job A");
    assert!(matches!(settled_a, State::Finished(Outcome::Completed)));
    let claim_b = queue
        .claim(Instant::now())
        .expect("claim call")
        .expect("job B is eligible after A settles");
    assert_eq!(claim_b.job(), submitted_b.job);
    let settled_b = queue
        .settle(
            &claim_b,
            key(
                "bound",
                "lib",
                "mount-a",
                "src",
                &version_b,
                &recipe_b,
                &processor_b,
            ),
            Outcome::Completed,
        )
        .expect("settle job B");
    assert!(matches!(settled_b, State::Finished(Outcome::Completed)));
    let after = queue.usage().expect("usage");
    assert_eq!(after.unsettled, before.unsettled);
    assert_eq!(after.pending, before.pending);

    queue
        .retire(
            submitted_a.job,
            key(
                "bound",
                "lib",
                "mount-a",
                "src",
                &version_a,
                &recipe_a,
                &processor_a,
            ),
        )
        .expect("retire job A");
    queue
        .retire(
            submitted_b.job,
            key(
                "bound",
                "lib",
                "mount-a",
                "src",
                &version_b,
                &recipe_b,
                &processor_b,
            ),
        )
        .expect("retire job B");
}

#[test]
fn zero_capacity_closes_admission() {
    let _guard = serial();

    // Scoped queue zero capacity is observable in this process.
    let zero = Queue::new(Limits {
        max_pending: 0,
        max_records: 0,
        max_descriptor_bytes: 0,
        per_mount: 0,
        memory_bytes: 0,
        temporary_bytes: 0,
    })
    .expect("zero limits are a valid closed configuration");
    let version = digest(51);
    let recipe = digest(52);
    let processor = digest(53);
    let refused = match zero.submit(Submission {
        operation_id: "zero-op",
        job_id: "zero-job",
        key: key("zero", "lib", "m", "src", &version, &recipe, &processor),
        budget: budget(),
        reservation: Reservation {
            memory_bytes: 0,
            temporary_bytes: 0,
        },
        priority: Priority::Normal,
        max_attempts: 1,
    }) {
        Ok(_) => panic!("zero queue capacities must close admission"),
        Err(error) => error,
    };
    assert!(matches!(refused, JobError::Capacity));

    let inconsistent = Queue::new(Limits {
        max_pending: 2,
        max_records: 1,
        max_descriptor_bytes: 1,
        per_mount: 1,
        memory_bytes: 1,
        temporary_bytes: 1,
    });
    assert!(
        matches!(inconsistent, Err(JobError::Invalid)),
        "pending capacity above retained-record capacity is Invalid"
    );

    // A zero effective admission policy must be established in a fresh process,
    // because start is once per process. Re-exec the same test binary.
    if std::env::var_os(ZERO_CHILD_ENV).is_some() {
        zero_capacity_child_scenario();
        return;
    }
    let exe = std::env::current_exe().expect("test binary path");
    let output = std::process::Command::new(exe)
        .arg("--exact")
        .arg("zero_capacity_closes_admission")
        .arg("--test-threads=1")
        .env(ZERO_CHILD_ENV, "1")
        .output()
        .expect("spawn the zero-capacity child process");
    assert!(
        output.status.success(),
        "child zero-capacity scenario failed: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const ZERO_CHILD_ENV: &str = "OMNI_ADMISSION_ZERO_CHILD";

fn zero_capacity_child_scenario() {
    let owner = match Account::start(
        open_private_dir("zero"),
        Configuration {
            memory_bytes: Some(0),
            storage_bytes: Some(0),
        },
        scheduling(),
    ) {
        Ok(owner) => owner,
        Err(error) => panic!("zero closes capacity but must still establish the owner: {error:?}"),
    };
    let policy = owner.policy();
    assert_eq!(policy.configuration.memory_bytes, Some(0));
    assert_eq!(policy.configuration.storage_bytes, Some(0));
    assert_eq!(
        policy.memory_bytes, 0,
        "zero memory closes the whole ceiling"
    );
    assert_eq!(
        policy.storage_bytes, 0,
        "zero storage closes the whole ceiling"
    );

    let error = match owner.reserve(
        Reservation {
            memory_bytes: 1,
            temporary_bytes: 0,
        },
        Origin::BeforeVerification,
    ) {
        Ok(_) => panic!("zero memory capacity must refuse any positive memory need"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::TooLargeMemory));
    let error = match owner.reserve(
        Reservation {
            memory_bytes: 0,
            temporary_bytes: 1,
        },
        Origin::BeforeVerification,
    ) {
        Ok(_) => panic!("zero storage capacity must refuse any positive storage need"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::TooLargeStorage));

    let before = owner.status().expect("status");
    assert_eq!(before.reserved.memory_bytes, 0);
    assert_eq!(before.reserved.temporary_bytes, 0);
}

#[test]
fn origin_job_and_split_claim_keep_resources_after_worker_and_mount_settlement() {
    let _guard = serial();
    let owner = account();
    let queue = owner.queue();
    let before = owner.status().expect("status");

    let version = synthetic_digest(b"public-admission-version-a");
    let recipe = synthetic_digest(b"public-admission-recipe-a");
    let processor = synthetic_digest(b"public-admission-processor-a");
    let key_a = key(
        "joint",
        "lib",
        "mount-origin",
        "src/version-a",
        &version,
        &recipe,
        &processor,
    );
    let submitted = owner
        .submit(Submission {
            operation_id: "op-origin-a",
            job_id: "job-origin-a",
            key: key_a,
            budget: budget(),
            reservation: Reservation {
                memory_bytes: MIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit job a");
    assert!(submitted.created);
    let claim = queue
        .claim(Instant::now())
        .expect("claim call")
        .expect("job a is eligible");
    assert_eq!(claim.job(), submitted.job);

    let claimed = owner.status().expect("status");
    assert_eq!(
        claimed.reserved.memory_bytes,
        before.reserved.memory_bytes + MIB,
        "the claim is charged to the same account"
    );
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert_eq!(inspected.execution.memory_bytes, MIB);
    assert!(inspected.waiting.is_none());

    // Origin::Job attributes a concurrent job-caused cost to the live claim.
    let mut origin_holding = match owner.reserve(
        Reservation {
            memory_bytes: 512 * KIB,
            temporary_bytes: 0,
        },
        Origin::Job {
            claim: &claim,
            expected: key_a,
        },
    ) {
        Ok(Decision::Ready(holding)) => holding,
        Ok(Decision::Waiting(_)) => panic!("job-caused cost fits current availability"),
        Err(error) => panic!("job-caused cost inside whole ceiling must not refuse: {error:?}"),
    };

    // Split part of the live claim to an independent resource lifetime; total stays the same.
    let split_bytes = 256 * KIB;
    let mut split_holding = owner
        .split_claim(
            &claim,
            key_a,
            Reservation {
                memory_bytes: split_bytes,
                temporary_bytes: 0,
            },
        )
        .expect("split a live claim");
    assert_eq!(
        split_holding.remaining().expect("remaining").memory_bytes,
        split_bytes
    );
    let split = owner.status().expect("status");
    assert_eq!(
        split.reserved.memory_bytes,
        before.reserved.memory_bytes + MIB + 512 * KIB,
        "split_claim must not double charge or leave a gap"
    );
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert_eq!(
        inspected.execution.memory_bytes,
        MIB - split_bytes,
        "the split portion no longer belongs to the mount execution"
    );

    // Actual executor settlement releases the remaining execution reservation and mount slot,
    // while the split/origin resources remain charged.
    let state = queue
        .settle(&claim, key_a, Outcome::Completed)
        .expect("settle job a");
    assert!(matches!(state, State::Finished(Outcome::Completed)));
    let settled = owner.status().expect("status");
    assert_eq!(
        settled.reserved.memory_bytes,
        before.reserved.memory_bytes + 512 * KIB + split_bytes,
        "resources transferred out of the claim remain charged after settlement"
    );

    // The mount slot is now free even though the split resource lives on.
    let version_b = synthetic_digest(b"public-admission-version-b");
    let recipe_b = synthetic_digest(b"public-admission-recipe-b");
    let processor_b = synthetic_digest(b"public-admission-processor-b");
    let key_b = key(
        "joint",
        "lib",
        "mount-origin",
        "src/version-b",
        &version_b,
        &recipe_b,
        &processor_b,
    );
    let submitted_b = owner
        .submit(Submission {
            operation_id: "op-origin-b",
            job_id: "job-origin-b",
            key: key_b,
            budget: budget(),
            reservation: Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit job b");
    let claim_b = queue
        .claim(Instant::now())
        .expect("claim call")
        .expect("mount slot is free after settlement");
    assert_eq!(claim_b.job(), submitted_b.job);
    queue
        .settle(&claim_b, key_b, Outcome::Completed)
        .expect("settle job b");

    origin_holding.finish().expect("finish origin cost");
    split_holding.finish().expect("finish split resource");
    let after = owner.status().expect("status");
    assert_eq!(
        after.reserved.memory_bytes, before.reserved.memory_bytes,
        "the trusted owner releases the independent promises"
    );

    queue.retire(submitted.job, key_a).expect("retire job a");
    queue.retire(submitted_b.job, key_b).expect("retire job b");
}

#[test]
fn grow_checks_the_complete_whole_job_need_against_whole_ceiling() {
    let _guard = serial();
    let owner = account();
    let before = owner.status().expect("status");

    let memory_available = before.memory_available;
    assert!(
        memory_available > 8 * KIB,
        "enough memory for a split scenario"
    );
    let half = memory_available / 2;
    let quarter = memory_available / 4;
    let mut growing = ready_holding(
        owner,
        Reservation {
            memory_bytes: half,
            temporary_bytes: 0,
        },
    );
    let mut other = ready_holding(
        owner,
        Reservation {
            memory_bytes: quarter,
            temporary_bytes: 0,
        },
    );
    let held = owner.status().expect("status");
    let current_available = held.memory_available;
    let charged = held.reserved.memory_bytes;
    assert!(current_available > 0, "the two holdings leave a remainder");

    // The delta alone is small, but the aggregate stays within the whole ceiling
    // while current availability blocks admission: visible Waiting, nothing changes.
    match growing.grow(Reservation {
        memory_bytes: current_available + 1,
        temporary_bytes: 0,
    }) {
        Ok(Decision::Waiting(Waiting::Memory)) => {}
        Ok(Decision::Waiting(Waiting::Storage)) => {
            panic!("memory growth contention must be Waiting(Memory)")
        }
        Ok(Decision::Ready(())) => panic!("growth above current availability must not be admitted"),
        Err(error) => {
            panic!("aggregate need within the whole ceiling must wait, not refuse: {error:?}")
        }
    }
    assert_eq!(
        growing.remaining().expect("remaining").memory_bytes,
        half,
        "Waiting changes nothing"
    );
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged
    );

    // The same delta is small, but the complete job need now exceeds the whole ceiling.
    let too_large_delta = WHOLE_MEMORY - half + 1;
    let error = match growing.grow(Reservation {
        memory_bytes: too_large_delta,
        temporary_bytes: 0,
    }) {
        Ok(_) => panic!("the aggregate need above the whole ceiling must be refused"),
        Err(error) => error,
    };
    assert!(
        matches!(error, Error::TooLargeMemory),
        "the complete causing-job need, not merely the delta, is checked against the whole ceiling"
    );
    assert_eq!(
        growing.remaining().expect("remaining").memory_bytes,
        half,
        "a typed refusal changes nothing"
    );
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged
    );

    growing.finish().expect("finish growing");
    other.finish().expect("finish other");
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes
    );

    // Generated-storage growth has the same aggregate-vs-availability distinction.
    let storage_available = owner.status().expect("status").storage_available;
    assert!(
        storage_available > 8 * KIB,
        "enough storage for a split scenario"
    );
    let storage_half = storage_available / 2;
    let mut storage_growing = ready_holding(
        owner,
        Reservation {
            memory_bytes: 0,
            temporary_bytes: storage_half,
        },
    );
    let storage_after_half = owner.status().expect("status").storage_available;
    let storage_quarter = storage_after_half / 2;
    let mut storage_other = ready_holding(
        owner,
        Reservation {
            memory_bytes: 0,
            temporary_bytes: storage_quarter,
        },
    );
    let storage_held = owner.status().expect("status");
    let storage_current = storage_held.storage_available;
    let storage_charged = storage_held.reserved.temporary_bytes;
    assert!(storage_current > 0);
    match storage_growing.grow(Reservation {
        memory_bytes: 0,
        temporary_bytes: storage_current + 1,
    }) {
        Ok(Decision::Waiting(Waiting::Storage)) => {}
        Ok(Decision::Waiting(Waiting::Memory)) => {
            panic!("storage growth contention must be Waiting(Storage)")
        }
        Ok(Decision::Ready(())) => {
            panic!("storage growth above current availability must not be admitted")
        }
        Err(error) => panic!("storage aggregate within whole ceiling must wait: {error:?}"),
    }
    assert_eq!(
        storage_growing
            .remaining()
            .expect("remaining")
            .temporary_bytes,
        storage_half,
        "Waiting changes nothing"
    );
    let storage_too_large = WHOLE_STORAGE - storage_half + 1;
    let error = match storage_growing.grow(Reservation {
        memory_bytes: 0,
        temporary_bytes: storage_too_large,
    }) {
        Ok(_) => panic!("storage aggregate above the whole ceiling must be refused"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::TooLargeStorage));
    assert_eq!(
        owner.status().expect("status").reserved.temporary_bytes,
        storage_charged
    );
    storage_growing.finish().expect("finish storage growing");
    storage_other.finish().expect("finish storage other");
    assert_eq!(
        owner.status().expect("status").reserved.temporary_bytes,
        before.reserved.temporary_bytes
    );
}

#[test]
fn wrong_queue_and_expected_claim_cannot_release_other_resources() {
    let _guard = serial();
    let owner = account();
    let queue = owner.queue();
    let before = owner.status().expect("status");

    // A standalone queue is a distinct actual instance; its claim cannot settle here.
    let standalone = Queue::new(Limits {
        max_pending: 16,
        max_records: 32,
        max_descriptor_bytes: 8192,
        per_mount: 1,
        memory_bytes: 4 * MIB,
        temporary_bytes: 64 * KIB,
    })
    .expect("standalone queue");
    let version = synthetic_digest(b"public-admission-foreign-version");
    let recipe = synthetic_digest(b"public-admission-foreign-recipe");
    let processor = synthetic_digest(b"public-admission-foreign-processor");
    let foreign_key = key(
        "joint",
        "lib",
        "foreign-queue",
        "src/foreign",
        &version,
        &recipe,
        &processor,
    );
    let foreign_submission = standalone
        .submit(Submission {
            operation_id: "op-foreign",
            job_id: "job-foreign",
            key: foreign_key,
            budget: budget(),
            reservation: Reservation {
                memory_bytes: MIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit foreign job");
    let foreign = standalone
        .claim(Instant::now())
        .expect("claim foreign job")
        .expect("foreign job eligible");
    assert_eq!(foreign.job(), foreign_submission.job);

    let error = match queue.settle(&foreign, foreign_key, Outcome::Completed) {
        Ok(_) => panic!("a claim owned by another queue must not settle through this queue"),
        Err(error) => error,
    };
    assert!(matches!(error, JobError::WrongInstance));
    assert_eq!(
        standalone.usage().expect("usage").unsettled,
        1,
        "the foreign claim remains unsettled"
    );
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes,
        "a wrong-instance settlement releases nothing here"
    );

    // split_claim also refuses a claim owned by another queue and charges nothing.
    let error = match owner.split_claim(
        &foreign,
        foreign_key,
        Reservation {
            memory_bytes: 1,
            temporary_bytes: 0,
        },
    ) {
        Ok(_) => panic!("a foreign claim cannot be transferred into this account"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Stale));
    assert_eq!(standalone.usage().expect("usage").unsettled, 1);
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes
    );

    standalone
        .settle(&foreign, foreign_key, Outcome::Completed)
        .expect("settle the foreign claim in its own owner");
    standalone
        .retire(foreign_submission.job, foreign_key)
        .expect("retire the foreign job");

    // A wrong expected key on a real claim must be refused without releasing the claim
    // or the independent holding that is still charged.
    let version_a = synthetic_digest(b"public-admission-context-version");
    let recipe_a = synthetic_digest(b"public-admission-context-recipe");
    let processor_a = synthetic_digest(b"public-admission-context-processor");
    let key_a = key(
        "joint",
        "lib",
        "context",
        "src/context-a",
        &version_a,
        &recipe_a,
        &processor_a,
    );
    let submitted = owner
        .submit(Submission {
            operation_id: "op-context",
            job_id: "job-context",
            key: key_a,
            budget: budget(),
            reservation: Reservation {
                memory_bytes: MIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit context job");
    let claim = queue
        .claim(Instant::now())
        .expect("claim context job")
        .expect("context job eligible");
    let mut other = ready_holding(
        owner,
        Reservation {
            memory_bytes: 512 * KIB,
            temporary_bytes: 0,
        },
    );
    let charged = owner.status().expect("status").reserved.memory_bytes;
    let wrong_key = key(
        "joint",
        "lib",
        "context",
        "src/context-other",
        &version_a,
        &recipe_a,
        &processor_a,
    );
    let error = match queue.settle(&claim, wrong_key, Outcome::Completed) {
        Ok(_) => panic!("a stale expected key must not settle"),
        Err(error) => error,
    };
    assert!(matches!(error, JobError::Context));
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged,
        "a refused settlement releases nothing, including the other holding"
    );
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert!(
        matches!(inspected.queue.state, State::Unsettled { .. }),
        "the claim remains unsettled after a refused settlement"
    );

    let state = queue
        .settle(&claim, key_a, Outcome::Completed)
        .expect("settle the claim with its real key");
    assert!(matches!(state, State::Finished(Outcome::Completed)));
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged - MIB,
        "exactly the claim's reservation is released, not the independent holding"
    );

    // A stale (already settled) claim cannot release anything else either.
    let error = match queue.settle(&claim, key_a, Outcome::Completed) {
        Ok(_) => panic!("an already settled claim must not settle again"),
        Err(error) => error,
    };
    assert!(matches!(error, JobError::StaleClaim));
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged - MIB,
        "the failed second settlement releases nothing"
    );

    other.finish().expect("finish independent holding");
    queue
        .retire(submitted.job, key_a)
        .expect("retire context job");
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes
    );
}

#[test]
fn holding_finish_releases_exactly_once() {
    let _guard = serial();
    let owner = account();
    let before = owner.status().expect("status");
    let mut holding = ready_holding(
        owner,
        Reservation {
            memory_bytes: MIB,
            temporary_bytes: 0,
        },
    );
    let charged = owner.status().expect("status");
    assert_eq!(
        charged.reserved.memory_bytes,
        before.reserved.memory_bytes + MIB
    );

    holding
        .finish()
        .expect("the trusted owner's first finish releases");
    let finished = owner.status().expect("status");
    assert_eq!(
        finished.reserved.memory_bytes, before.reserved.memory_bytes,
        "finish releases the retained promise"
    );
    assert_eq!(
        finished.holdings, before.holdings,
        "a finished capability no longer occupies a live holding record"
    );

    let error = match holding.finish() {
        Ok(_) => panic!("a finished capability must not release again"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Stale));
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes,
        "the second finish releases nothing"
    );

    let error = match holding.remaining() {
        Ok(_) => panic!("a finished capability has no remaining promises"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Stale));
    let error = match holding.grow(Reservation {
        memory_bytes: 1,
        temporary_bytes: 0,
    }) {
        Ok(_) => panic!("a finished capability cannot grow"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Stale));
    let error = match holding.written(1) {
        Ok(_) => panic!("a finished capability cannot reconcile a write"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Stale));
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes,
        "lost capabilities keep no charge"
    );
}

#[test]
fn written_reconciles_actual_native_bytes_after_a_real_writer() {
    let _guard = serial();
    let owner = account();
    let before = owner.status().expect("status");
    let policy = owner.policy();
    let scope = PRIMARY_DIR.get().expect("primary directory path").clone();
    let promise = 16 * KIB;
    let mut holding = ready_holding(
        owner,
        Reservation {
            memory_bytes: 0,
            temporary_bytes: promise,
        },
    );
    let charged = owner.status().expect("status");
    assert_eq!(
        charged.reserved.temporary_bytes,
        before.reserved.temporary_bytes + promise
    );

    // Each call is a newly confirmed delta from an actual completed write in the selected
    // directory, never a replay or a cumulative count and never a synthetic I/O counter.
    let first_path = scope.join("written-first.bin");
    let first_payload = vec![0x5Au8; (4 * KIB) as usize];
    std::fs::write(&first_path, &first_payload).expect("real native write");
    let first_len = std::fs::metadata(&first_path)
        .expect("metadata for the completed write")
        .len();
    assert_eq!(first_len, first_payload.len() as u64);
    let volume = holding
        .written(first_len)
        .expect("reconcile the completed byte delta");
    assert_eq!(volume.device, policy.startup_volume.device);
    assert_eq!(volume.directory, policy.startup_volume.directory);
    assert_eq!(volume.capacity_bytes, policy.startup_volume.capacity_bytes);
    assert_eq!(volume.floor_bytes, policy.startup_volume.floor_bytes);
    assert!(volume.available_bytes <= volume.capacity_bytes);
    assert_eq!(
        holding.remaining().expect("remaining").temporary_bytes,
        promise - first_len,
        "only the unwritten promise stays charged"
    );
    assert_eq!(
        owner.status().expect("status").reserved.temporary_bytes,
        before.reserved.temporary_bytes + promise - first_len
    );

    let second_path = scope.join("written-second.bin");
    let second_payload = vec![0xA5u8; (2 * KIB) as usize];
    std::fs::write(&second_path, &second_payload).expect("real native write");
    let second_len = std::fs::metadata(&second_path)
        .expect("metadata for the second completed write")
        .len();
    assert_eq!(second_len, second_payload.len() as u64);
    let volume = holding
        .written(second_len)
        .expect("reconcile the new completed delta");
    assert_eq!(volume.device, policy.startup_volume.device);
    assert_eq!(volume.directory, policy.startup_volume.directory);
    assert_eq!(
        holding.remaining().expect("remaining").temporary_bytes,
        promise - first_len - second_len
    );
    assert_eq!(
        owner.status().expect("status").reserved.temporary_bytes,
        before.reserved.temporary_bytes + promise - first_len - second_len
    );

    holding.finish().expect("finish the reconciled storage");
    assert_eq!(
        owner.status().expect("status").reserved.temporary_bytes,
        before.reserved.temporary_bytes,
        "finish releases the remaining unwritten promise"
    );
    std::fs::remove_file(&first_path).expect("remove the first synthetic file");
    std::fs::remove_file(&second_path).expect("remove the second synthetic file");
}

#[test]
fn simultaneous_holdings_status_and_claim_stay_consistent() {
    let _guard = serial();
    let owner = account();
    let queue = owner.queue();
    let before = owner.status().expect("status");
    let before_usage = queue.usage().expect("usage");

    let version = synthetic_digest(b"public-admission-atomic-version");
    let recipe = synthetic_digest(b"public-admission-atomic-recipe");
    let processor = synthetic_digest(b"public-admission-atomic-processor");
    let key_a = key(
        "joint",
        "lib",
        "atomic",
        "src/atomic",
        &version,
        &recipe,
        &processor,
    );
    let submitted = owner
        .submit(Submission {
            operation_id: "op-atomic",
            job_id: "job-atomic",
            key: key_a,
            budget: budget(),
            reservation: Reservation {
                memory_bytes: MIB,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .expect("submit atomic job");
    let claim = queue
        .claim(Instant::now())
        .expect("claim atomic job")
        .expect("atomic job eligible");
    let mut first = ready_holding(
        owner,
        Reservation {
            memory_bytes: 2 * MIB,
            temporary_bytes: 0,
        },
    );
    let mut second = ready_holding(
        owner,
        Reservation {
            memory_bytes: MIB,
            temporary_bytes: 0,
        },
    );

    let joined = owner.status().expect("status");
    assert_eq!(
        joined.reserved.memory_bytes,
        before.reserved.memory_bytes + MIB + 3 * MIB,
        "account total is exactly the claim plus the independent holdings"
    );
    assert_eq!(
        joined.queue.reserved.memory_bytes, MIB,
        "the queue projection holds only the unsettled execution portion"
    );
    assert_eq!(joined.holdings, before.holdings + 2);
    let usage = queue.usage().expect("usage");
    assert_eq!(usage.unsettled, before_usage.unsettled + 1);
    assert_eq!(usage.reserved.memory_bytes, MIB);
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert_eq!(inspected.execution.memory_bytes, MIB);

    let available = joined.memory_available;
    let charged = joined.reserved.memory_bytes;
    // A grow that would keep the aggregate inside the whole ceiling but exceed
    // current availability is visible Waiting and changes nothing anywhere.
    match first.grow(Reservation {
        memory_bytes: available + 1,
        temporary_bytes: 0,
    }) {
        Ok(Decision::Waiting(Waiting::Memory)) => {}
        Ok(Decision::Waiting(Waiting::Storage)) => {
            panic!("memory contention must be Waiting(Memory)")
        }
        Ok(Decision::Ready(())) => panic!("growth above availability must not be admitted"),
        Err(error) => panic!("aggregate inside whole ceiling must wait: {error:?}"),
    }
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        charged
    );
    assert_eq!(queue.usage().expect("usage").reserved.memory_bytes, MIB);
    assert_eq!(first.remaining().expect("remaining").memory_bytes, 2 * MIB);
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert_eq!(inspected.execution.memory_bytes, MIB);
    assert_eq!(inspected.queue.reservation.memory_bytes, MIB);

    // Split a live claim atomically: the account total is unchanged while ownership
    // moves from the queue execution to an independent holding.
    let split_bytes = 256 * KIB;
    let mut split = owner
        .split_claim(
            &claim,
            key_a,
            Reservation {
                memory_bytes: split_bytes,
                temporary_bytes: 0,
            },
        )
        .expect("split the live claim");
    let moved = owner.status().expect("status");
    assert_eq!(
        moved.reserved.memory_bytes, charged,
        "a transfer moves ownership without double charging or a gap"
    );
    assert_eq!(moved.holdings, before.holdings + 3);
    assert_eq!(
        moved.queue.reserved.memory_bytes,
        MIB - split_bytes,
        "the queue keeps only the execution-owned remainder"
    );
    let usage = queue.usage().expect("usage");
    assert_eq!(usage.reserved.memory_bytes, MIB - split_bytes);
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert_eq!(inspected.execution.memory_bytes, MIB - split_bytes);

    let state = queue
        .settle(&claim, key_a, Outcome::Completed)
        .expect("settle the execution-owned remainder");
    assert!(matches!(state, State::Finished(Outcome::Completed)));
    let settled = owner.status().expect("status");
    assert_eq!(
        settled.reserved.memory_bytes,
        before.reserved.memory_bytes + 3 * MIB + split_bytes,
        "only the execution-owned remainder is released"
    );
    assert_eq!(
        settled.queue.reserved.memory_bytes, 0,
        "no unsettled execution portion remains"
    );
    assert_eq!(queue.usage().expect("usage").reserved.memory_bytes, 0);
    assert_eq!(
        settled.holdings,
        before.holdings + 3,
        "split and independent holdings live independently of the claim"
    );
    let inspected = owner.job_status(submitted.job, key_a).expect("job status");
    assert_eq!(inspected.execution.memory_bytes, 0);
    assert!(inspected.waiting.is_none());

    first.finish().expect("finish first holding");
    second.finish().expect("finish second holding");
    split.finish().expect("finish split holding");
    queue
        .retire(submitted.job, key_a)
        .expect("retire atomic job");
    assert_eq!(
        owner.status().expect("status").reserved.memory_bytes,
        before.reserved.memory_bytes
    );
}
