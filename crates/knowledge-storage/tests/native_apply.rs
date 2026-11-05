//! Builder functional checks against actual native files/Git and the actual shared account.
use knowledge_core::{
    admission::{Account, Configuration, Decision, Origin, Scheduling},
    backend::Budget,
    data::ContentDigest,
    jobs::*,
};
use knowledge_history::{Change, Destination, History, Mode, OwnerContext, Request};
use knowledge_storage::*;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
const M: u64 = 1024 * 1024;
/// Serialize only fixture ownership of this process-global account; explicit race cases remain real races.
static SERIAL: Mutex<()> = Mutex::new(());
/// Keep the selected actual cache directory alive as long as the process-owned descriptor.
static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
/// Fresh current native fixture owner; no manifest/wire data is interpreted as permission.
#[derive(Clone)]
struct Policy(Arc<AtomicU64>);
impl WritePolicy for Policy {
    /// Return the explicitly admitted fixture root context with a live revision.
    fn current(&self) -> Result<WriteContext, Error> {
        Ok(WriteContext {
            library_id: "lib".into(),
            revision: self.0.load(Ordering::Acquire),
            text_root: "pages".into(),
            writable: true,
            raw_original_writes: false,
            outside_scope: vec![],
        })
    }
}
/// Own the exact temporary home/repository and actual unsettled process claim through all children/I/O.
struct Fixture {
    /// Lifetime fixture serialization guard, not production scheduling.
    _serial: MutexGuard<'static, ()>,
    /// Temporary parent retained until all Git/file work settles.
    parent: tempfile::TempDir,
    /// Verified actual marker digest for this execution key.
    version: ContentDigest,
    /// Actual fixed builder recipe bytes hashed, not a fake original digest.
    recipe: ContentDigest,
    /// Process account, never a fixture-created second pool.
    account: &'static Account,
    /// Exact current execution capability.
    claim: Claim,
    /// Current native policy provider.
    policy: Policy,
}
impl Fixture {
    /// Bind actual disposable files, reserve before hashing and claim through the one process account.
    fn new() -> Self {
        let serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let cache = CACHE.get_or_init(|| {
            let t = tempfile::tempdir().unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(t.path(), fs::Permissions::from_mode(0o700)).unwrap();
            t
        });
        let account = match Account::current() {
            Ok(a) => a,
            Err(_) => Account::start(
                File::open(cache.path()).unwrap(),
                Configuration {
                    memory_bytes: Some(512 * M),
                    storage_bytes: Some(1024 * M),
                },
                Scheduling {
                    max_pending: 128,
                    max_records: 1024,
                    max_descriptor_bytes: 4 * M as usize,
                    per_mount: 16,
                },
            )
            .unwrap(),
        };
        let parent = tempfile::Builder::new()
            .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
            .unwrap();
        fs::create_dir(parent.path().join("home")).unwrap();
        fs::write(parent.path().join("home/omni-knowledge.json"),b"{\"version\":2,\"library_id\":\"lib\",\"name\":\"fixture\",\"text_root\":\"pages\",\"mounts\":[],\"convention_version\":\"0.2\",\"exposure\":{\"mode\":\"off\",\"sensitive\":false,\"excluded_paths\":[],\"transports\":[]}}").unwrap();
        let mut pre = match account
            .reserve(
                Reservation {
                    memory_bytes: 65536,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )
            .unwrap()
        {
            Decision::Ready(h) => h,
            _ => panic!("fixture hash contention"),
        };
        let bytes = fs::read(parent.path().join("home/omni-knowledge.json")).unwrap();
        let version = ContentDigest::new(format!("{:x}", Sha256::digest(&bytes))).unwrap();
        drop(bytes);
        pre.finish().unwrap();
        let recipe = ContentDigest::new(format!(
            "{:x}",
            Sha256::digest(b"native storage builder functional gate")
        ))
        .unwrap();
        let context = parent.path().to_str().unwrap();
        let key = Key {
            context,
            library: "lib",
            mount: None,
            source: "omni-knowledge.json",
            version: &version,
            recipe: &recipe,
            processor: &recipe,
        };
        account
            .submit(Submission {
                operation_id: context,
                job_id: context,
                key,
                budget: Budget {
                    max_files: 128,
                    max_bytes: 8 * M,
                    max_millis: 60000,
                    max_memory_bytes: 96 * M,
                },
                reservation: Reservation {
                    memory_bytes: 96 * M,
                    temporary_bytes: 0,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        Self {
            _serial: serial,
            parent,
            version,
            recipe,
            account,
            claim,
            policy: Policy(Arc::new(AtomicU64::new(0))),
        }
    }
    /// Borrow exact existing key facts without a new digest/permission constructor.
    fn key(&self) -> Key<'_> {
        Key {
            context: self.parent.path().to_str().unwrap(),
            library: "lib",
            mount: None,
            source: "omni-knowledge.json",
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.recipe,
        }
    }
    /// Lend explicit filesystem scratch from the existing admission claim.
    fn scope(&self) -> Scope<'_> {
        Scope {
            claim: &self.claim,
            key: self.key(),
            scratch_bytes: 16 * M,
        }
    }
    /// Select actual native current home.
    fn home(&self) -> std::path::PathBuf {
        self.parent.path().join("home")
    }
    /// Open the real selected store without any fake filesystem implementation.
    fn store(&self) -> Store<Policy> {
        Store::open(&self.home(), "lib", "writer-a", self.policy.clone()).unwrap()
    }
    /// Create a real external bare repository under explicit unsigned fixture context.
    fn history(&self) -> History {
        History::init(
            &self.parent.path().join("repo"),
            &self.home(),
            "pages",
            OwnerContext {
                name: "Fixture Owner".into(),
                email: "fixture@example.invalid".into(),
                agent: None,
                tool: None,
                session: None,
                mode: Mode::UnsignedExplicit,
            },
            &self.claim,
        )
        .unwrap()
    }
    /// Reserve actual pre-verification work without a fabricated original digest/job key.
    fn preflight_holding(&self) -> knowledge_core::admission::Holding {
        match self
            .account
            .reserve(
                Reservation {
                    memory_bytes: 16 * M,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )
            .unwrap()
        {
            Decision::Ready(h) => h,
            _ => panic!("fixture preflight contention"),
        }
    }
}
impl Drop for Fixture {
    /// All test Git calls collect exact children synchronously; return scratch and settle before cleanup.
    fn drop(&mut self) {
        let _ = self
            .account
            .queue()
            .settle(&self.claim, self.key(), Outcome::Completed);
        let _ = self.account.queue().retire(self.claim.job(), self.key());
    }
}
/// Build a selected real Git delta without a proposal/authority shortcut.
fn request(
    id: &str,
    base: Option<knowledge_history::Oid>,
    values: &[(&str, Option<&str>)],
) -> Request {
    Request {
        operation_id: id.into(),
        base,
        destination: Destination::Accepted,
        changes: values
            .iter()
            .map(|(p, t)| Change {
                path: (*p).into(),
                text: t.map(str::to_owned),
            })
            .collect(),
    }
}

/// Actual preparation/install/publication/reopen proves bytes AND Git AND the visible saved record.
#[test]
fn actual_saved_reopens_and_duplicate_delivery_does_not_reinstall() {
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    let req = request("first", None, &[("pages/a.md", Some("one\n"))]);
    let expected = [Expected {
        path: "pages/a.md".into(),
        version: None,
    }];
    assert_eq!(
        store
            .prepare(&history, &req, &expected, &scope)
            .unwrap()
            .phase,
        Phase::Prepared
    );
    assert!(!f.home().join("pages/a.md").exists());
    assert_eq!(
        store.install(&history, "first", &scope).unwrap().phase,
        Phase::FilesInstalled
    );
    assert_eq!(
        store.publish(&history, "first", &scope).unwrap().phase,
        Phase::SavedLocal
    );
    assert_eq!(fs::read(f.home().join("pages/a.md")).unwrap(), b"one\n");
    assert_eq!(
        store
            .prepare(&history, &req, &expected, &scope)
            .unwrap()
            .phase,
        Phase::SavedLocal
    );
    assert_eq!(
        f.store().inspect(&history, "first", &scope).unwrap().phase,
        Phase::SavedLocal
    );
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, 96 * M);
    assert_eq!(f.account.status().unwrap().holdings, 0);
}
/// A real competing destination after preparation is preserved with the exact candidate untouched.
#[test]
fn competing_create_is_not_clobbered() {
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    let req = request("compete", None, &[("pages/a.md", Some("candidate\n"))]);
    let expected = [Expected {
        path: "pages/a.md".into(),
        version: None,
    }];
    store.prepare(&history, &req, &expected, &scope).unwrap();
    fs::create_dir(f.home().join("pages")).unwrap();
    fs::write(f.home().join("pages/a.md"), b"competitor\n").unwrap();
    assert!(matches!(
        store.install(&history, "compete", &scope),
        Err(Error::Conflict)
    ));
    assert_eq!(
        fs::read(f.home().join("pages/a.md")).unwrap(),
        b"competitor\n"
    );
    assert!(history.head(&f.claim).unwrap().is_none());
}
/// Full expected digest/stat catches an actual edit after observation and before preparation.
#[test]
fn stale_expected_does_not_replace_changed_original() {
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    fs::create_dir(f.home().join("pages")).unwrap();
    fs::write(f.home().join("pages/a.md"), b"old\n").unwrap();
    let old = store.observe("pages/a.md", &scope).unwrap();
    fs::write(f.home().join("pages/a.md"), b"new outside\n").unwrap();
    let req = request("stale", None, &[("pages/a.md", Some("candidate\n"))]);
    assert!(matches!(
        store.prepare(
            &history,
            &req,
            &[Expected {
                path: "pages/a.md".into(),
                version: old
            }],
            &scope
        ),
        Err(Error::Conflict)
    ));
    assert_eq!(
        fs::read(f.home().join("pages/a.md")).unwrap(),
        b"new outside\n"
    );
}
/// An actual second-path failure leaves first-path bytes inspectable and explicit resume completes safely.
#[test]
fn interrupted_multi_file_install_reopens_without_rollback() {
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    let req = request(
        "partial",
        None,
        &[("pages/a.md", Some("a\n")), ("pages/z/b.md", Some("b\n"))],
    );
    let expected = [
        Expected {
            path: "pages/a.md".into(),
            version: None,
        },
        Expected {
            path: "pages/z/b.md".into(),
            version: None,
        },
    ];
    store.prepare(&history, &req, &expected, &scope).unwrap();
    fs::create_dir(f.home().join("pages")).unwrap();
    fs::write(f.home().join("pages/z"), b"blocking outside file").unwrap();
    assert!(store.install(&history, "partial", &scope).is_err());
    assert_eq!(
        store.inspect(&history, "partial", &scope).unwrap().phase,
        Phase::Indeterminate
    );
    assert_eq!(fs::read(f.home().join("pages/a.md")).unwrap(), b"a\n");
    fs::rename(
        f.home().join("pages/z"),
        f.home().join("retained-outside-file"),
    )
    .unwrap();
    let reopened = f.store();
    assert_eq!(
        reopened.install(&history, "partial", &scope).unwrap().phase,
        Phase::FilesInstalled
    );
    assert_eq!(
        reopened.publish(&history, "partial", &scope).unwrap().phase,
        Phase::SavedLocal
    );
    assert_eq!(
        fs::read(f.home().join("retained-outside-file")).unwrap(),
        b"blocking outside file"
    );
}
/// Known mutable-v2 bytes retain opaque fields exactly, never a lossy metadata projection.
const MARKER:&[u8]=b"{\"version\":2,\"library_id\":\"lib\",\"name\":\"new\",\"text_root\":\"pages\",\"mounts\":[],\"convention_version\":\"0.2\",\"exposure\":{\"mode\":\"off\",\"sensitive\":false,\"excluded_paths\":[],\"transports\":[]},\"opaque\":{\"exact\":1e2}}";
/// A real competing parent after staging causes a partial UNMARKED root; explicit reopen resumes safely.
#[test]
fn initializer_marker_last_and_explicit_recovery_preserve_competitor() {
    let f = Fixture::new();
    let root = f.parent.path().join("new-home");
    fs::create_dir(&root).unwrap();
    let store = Store::open(&root, "lib", "writer-a", f.policy.clone()).unwrap();
    let mut holding = f.preflight_holding();
    let pre = Preflight {
        holding: &holding,
        budget: f.claim.budget(),
    };
    let seeds = [
        Seed {
            path: "omni-knowledge.json",
            bytes: MARKER,
        },
        Seed {
            path: "pages/a.md",
            bytes: b"first\n",
        },
        Seed {
            path: "pages/z/b.md",
            bytes: b"second\n",
        },
    ];
    store.stage_initialization("init", &seeds, &pre).unwrap();
    assert!(!root.join("omni-knowledge.json").exists());
    fs::create_dir(root.join("pages")).unwrap();
    fs::write(root.join("pages/z"), b"competing file").unwrap();
    match store.resume_initialization("init", &pre).unwrap() {
        InitOutcome::Partial {
            marker_present,
            confirmed_installed,
            ..
        } => {
            assert_eq!(marker_present, Some(false));
            assert!(confirmed_installed.contains(&"pages/a.md".into()));
        }
        _ => panic!("unexpected complete initialization"),
    }
    assert!(!root.join("omni-knowledge.json").exists());
    fs::rename(root.join("pages/z"), root.join("retained-competitor")).unwrap();
    assert!(matches!(
        store.resume_initialization("init", &pre).unwrap(),
        InitOutcome::Initialized { .. }
    ));
    assert_eq!(fs::read(root.join("omni-knowledge.json")).unwrap(), MARKER);
    assert_eq!(
        fs::read(root.join("retained-competitor")).unwrap(),
        b"competing file"
    );
    assert!(matches!(
        store.inspect_initialization("init", &pre).unwrap(),
        InitOutcome::Initialized { .. }
    ));
    holding.finish().unwrap();
}
/// Colliding templates refuse before any payload or manifest is installed; original data survives.
#[test]
fn initializer_preflight_refuses_existing_destination() {
    let f = Fixture::new();
    let root = f.parent.path().join("collision-home");
    fs::create_dir_all(root.join("pages")).unwrap();
    fs::write(root.join("pages/a.md"), b"authored").unwrap();
    let store = Store::open(&root, "lib", "writer-a", f.policy.clone()).unwrap();
    let mut holding = f.preflight_holding();
    let pre = Preflight {
        holding: &holding,
        budget: f.claim.budget(),
    };
    assert!(matches!(
        store.initialize(
            "init-collision",
            &[
                Seed {
                    path: "omni-knowledge.json",
                    bytes: MARKER
                },
                Seed {
                    path: "pages/a.md",
                    bytes: b"candidate"
                }
            ],
            &pre
        ),
        Err(Error::Conflict)
    ));
    assert_eq!(fs::read(root.join("pages/a.md")).unwrap(), b"authored");
    assert!(!root.join("omni-knowledge.json").exists());
    assert!(!root.join("journal").exists());
    holding.finish().unwrap();
}
/// Actual copy verifies destination bytes, corrects marker naming and never deletes/mutates its source.
#[test]
fn import_copies_original_and_renames_reserved_marker() {
    let f = Fixture::new();
    let store = f.store();
    let source = f.parent.path().join("original.json");
    fs::write(&source, b"original bytes\n").unwrap();
    let source_file = File::open(&source).unwrap();
    let mut holding = f.preflight_holding();
    let pre = Preflight {
        holding: &holding,
        budget: f.claim.budget(),
    };
    let imported = store
        .import_copy(
            "copy",
            &source_file,
            "raw/managed/omni-knowledge.json",
            &pre,
        )
        .unwrap();
    assert_eq!(imported.path, "raw/managed/omni-knowledge.asset.json");
    assert_eq!(
        fs::read(f.home().join(&imported.path)).unwrap(),
        b"original bytes\n"
    );
    assert_eq!(fs::read(&source).unwrap(), b"original bytes\n");
    assert_eq!(
        store.inspect_import("copy", &pre).unwrap().version,
        imported.version
    );
    assert!(matches!(
        store.import_copy("copy-other", &source_file, &imported.path, &pre),
        Err(Error::Conflict)
    ));
    holding.finish().unwrap();
}

/// Explicit current native read owner for this already selected disposable home.
struct ReadOwner(Arc<AtomicU64>);
impl knowledge_local::LocalReadPolicy for ReadOwner {
    /// Real selected ordinary-local facts, never a manifest/wire grant.
    fn decide(&self) -> knowledge_local::ReadDecision {
        knowledge_local::ReadDecision::Admit(knowledge_local::CurrentAdmission {
            library_id: "lib".into(),
            mount_id: None,
            revision: self.0.load(Ordering::Acquire),
            locality: knowledge_local::LocalityPremise::OrdinaryLocal,
            outside_scope: vec![knowledge_core::data::RelativePath::new("journal").unwrap()],
        })
    }
}
/// Real pre-verification holding enables full hashing; settled holding refuses reuse.
#[test]
fn actual_local_verification_needs_no_fabricated_digest() {
    use knowledge_core::{
        backend::{BackendError, ByteRange, MountBackend},
        data::RelativePath,
    };
    let f = Fixture::new();
    fs::create_dir(f.home().join("pages")).unwrap();
    fs::write(f.home().join("pages/a.md"), b"source bytes").unwrap();
    let mount =
        knowledge_local::LocalMount::open(&f.home(), "lib", None, ReadOwner(f.policy.0.clone()))
            .unwrap();
    let mut holding = f.preflight_holding();
    let budget = Budget {
        max_memory_bytes: 8 * M,
        ..f.claim.budget()
    };
    let path = RelativePath::new("pages/a.md").unwrap();
    let version = mount.verify(&path, budget, &holding).unwrap();
    assert_eq!(
        version.digest.as_str(),
        format!(
            "{:x}",
            Sha256::digest(fs::read(f.home().join("pages/a.md")).unwrap())
        )
    );
    assert_eq!(
        mount
            .open_range(&version, ByteRange::new(0, 6).unwrap(), budget)
            .unwrap()
            .bytes,
        b"source"
    );
    holding.finish().unwrap();
    assert!(matches!(
        mount.verify(&path, budget, &holding),
        Err(BackendError::InvalidRequest)
    ));
}
/// Actual Linux inotify: startup/new-directory gaps require scans; close settles exact native resources.
#[cfg(target_os = "linux")]
#[test]
fn native_watch_is_hint_only_and_retains_actual_process_memory() {
    use knowledge_core::backend::{Enumeration, MountBackend};
    let f = Fixture::new();
    fs::create_dir(f.home().join("pages")).unwrap();
    fs::write(f.home().join("pages/a.md"), b"old").unwrap();
    let mount =
        knowledge_local::LocalMount::open(&f.home(), "lib", None, ReadOwner(f.policy.0.clone()))
            .unwrap();
    let baseline = f.account.status().unwrap().reserved.memory_bytes;
    let budget = Budget {
        max_memory_bytes: M,
        ..f.claim.budget()
    };
    let mut watch = mount.watch(budget).unwrap();
    assert_eq!(
        f.account.status().unwrap().reserved.memory_bytes,
        baseline + M
    );
    let initial = watch.poll().unwrap();
    assert!(initial.reconciliation_required && initial.coverage_complete);
    let actual = mount.list(None, budget).unwrap();
    assert!(matches!(actual, Enumeration::Complete { .. }));
    let mut inventory = File::create(f.parent.path().join("durable-watch-inventory.json")).unwrap();
    serde_json::to_writer(&mut inventory, &actual).unwrap();
    inventory.sync_all().unwrap();
    drop(inventory);
    assert!(
        !watch
            .acknowledge_reconciliation(initial.epoch)
            .unwrap()
            .reconciliation_required
    );
    fs::write(f.home().join("pages/a.md"), b"outside changed").unwrap();
    let changed = watch.poll().unwrap();
    assert!(changed.epoch > initial.epoch && changed.reconciliation_required);
    fs::create_dir(f.home().join("pages/new-directory")).unwrap();
    let gap = watch.poll().unwrap();
    assert!(!gap.coverage_complete && gap.reconciliation_required);
    assert!(
        watch
            .acknowledge_reconciliation(gap.epoch)
            .unwrap()
            .reconciliation_required
    );
    drop(watch);
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, baseline);
}

/// Actual Git publication without corresponding installed home bytes is never cross-domain saved_local.
#[test]
fn git_published_alone_does_not_prove_saved_local() {
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    let req = request("git-only", None, &[("pages/a.md", Some("candidate\n"))]);
    let p = store
        .prepare(
            &history,
            &req,
            &[Expected {
                path: "pages/a.md".into(),
                version: None,
            }],
            &scope,
        )
        .unwrap();
    assert!(p.newly_prepared);
    history.publish("git-only", &f.claim).unwrap();
    let observed = store.inspect(&history, "git-only", &scope).unwrap();
    assert_ne!(observed.phase, Phase::SavedLocal);
    assert!(!observed.current_matches);
    assert!(!f.home().join("pages/a.md").exists());
}
/// Actual replacement preserves displaced original bytes in visible same-filesystem recovery.
#[test]
fn replacement_preserves_actual_original_and_immutable_git() {
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    fs::create_dir(f.home().join("pages")).unwrap();
    fs::write(f.home().join("pages/a.md"), b"original\n").unwrap();
    let expected = [Expected {
        path: "pages/a.md".into(),
        version: store.observe("pages/a.md", &scope).unwrap(),
    }];
    let req = request("replace", None, &[("pages/a.md", Some("replacement\n"))]);
    assert_eq!(
        store
            .apply(&history, &req, &expected, &scope)
            .unwrap()
            .phase,
        Phase::SavedLocal
    );
    assert_eq!(
        fs::read(f.home().join("pages/a.md")).unwrap(),
        b"replacement\n"
    );
    let lane = format!("{:x}", Sha256::digest(b"replace"));
    assert_eq!(
        fs::read(f.home().join("journal/recovery").join(lane).join("old-0")).unwrap(),
        b"original\n"
    );
    assert_eq!(
        store
            .apply(&history, &req, &expected, &scope)
            .unwrap()
            .phase,
        Phase::SavedLocal
    );
}
/// Malformed native tail remains byte-for-byte retained; no new payload or false saved result is emitted.
#[test]
fn malformed_tail_is_reported_without_discard_or_repair() {
    use std::io::Write;
    let f = Fixture::new();
    let store = f.store();
    let history = f.history();
    let scope = f.scope();
    let req = request("before-tail", None, &[("pages/a.md", Some("saved\n"))]);
    let saved = store
        .apply(
            &history,
            &req,
            &[Expected {
                path: "pages/a.md".into(),
                version: None,
            }],
            &scope,
        )
        .unwrap();
    assert_eq!(saved.phase, Phase::SavedLocal);
    let path = f.home().join("journal/writer-a.jsonl");
    let mut outside = fs::OpenOptions::new().append(true).open(&path).unwrap();
    outside.write_all(b"{truncated").unwrap();
    outside.sync_all().unwrap();
    drop(outside);
    let damaged = fs::read(&path).unwrap();
    assert!(matches!(
        store.inspect(&history, "before-tail", &scope),
        Err(Error::Recovery)
    ));
    let later = request(
        "after-tail",
        history.head(&f.claim).unwrap(),
        &[("pages/b.md", Some("must not install\n"))],
    );
    assert!(matches!(
        store.prepare(
            &history,
            &later,
            &[Expected {
                path: "pages/b.md".into(),
                version: None
            }],
            &scope
        ),
        Err(Error::Recovery)
    ));
    assert_eq!(fs::read(&path).unwrap(), damaged);
    assert_eq!(fs::read(f.home().join("pages/a.md")).unwrap(), b"saved\n");
    assert!(!f.home().join("pages/b.md").exists());
}

/// Synchronous scratch returns preserve aggregate charge and refuse adoption by a later reconnect generation.
#[test]
fn synchronous_loan_rejoins_only_original_live_generation() {
    use knowledge_core::admission::Error as AdmissionError;
    let f = Fixture::new();
    let baseline = f.account.status().unwrap().reserved.memory_bytes;
    let mut loan = f
        .account
        .split_claim(
            &f.claim,
            f.key(),
            Reservation {
                memory_bytes: M,
                temporary_bytes: 0,
            },
        )
        .unwrap();
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, baseline);
    f.account
        .rejoin_claim(&mut loan, &f.claim, f.key())
        .unwrap();
    assert!(matches!(loan.remaining(), Err(AdmissionError::Stale)));
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, baseline);
    let recipe = ContentDigest::new(format!(
        "{:x}",
        Sha256::digest(b"actual scratch reconnect test recipe")
    ))
    .unwrap();
    let key = Key {
        recipe: &recipe,
        processor: &recipe,
        ..f.key()
    };
    f.account
        .submit(Submission {
            operation_id: "loan-reconnect",
            job_id: "loan-reconnect",
            key,
            budget: Budget {
                max_memory_bytes: 2 * M,
                ..f.claim.budget()
            },
            reservation: Reservation {
                memory_bytes: 2 * M,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 2,
        })
        .unwrap();
    let old = f
        .account
        .queue()
        .claim(std::time::Instant::now())
        .unwrap()
        .unwrap();
    let mut loan = f
        .account
        .split_claim(
            &old,
            key,
            Reservation {
                memory_bytes: M,
                temporary_bytes: 0,
            },
        )
        .unwrap();
    f.account
        .queue()
        .settle(&old, key, Outcome::Offline)
        .unwrap();
    f.account.queue().reconnect(old.job(), key).unwrap();
    let new = f
        .account
        .queue()
        .claim(std::time::Instant::now())
        .unwrap()
        .unwrap();
    let before = f.account.status().unwrap().reserved;
    assert!(matches!(
        f.account.rejoin_claim(&mut loan, &new, key),
        Err(AdmissionError::Stale)
    ));
    assert_eq!(f.account.status().unwrap().reserved, before);
    assert_eq!(loan.remaining().unwrap().memory_bytes, M);
    // No payload/worker was ever allocated for this promise-only contract case.
    loan.finish().unwrap();
    f.account
        .queue()
        .settle(&new, key, Outcome::Completed)
        .unwrap();
    f.account.queue().retire(new.job(), key).unwrap();
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, baseline);
}
