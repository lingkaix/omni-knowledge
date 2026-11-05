//! Independent public-contract tests for the portable cooperative jobs queue.
//!
//! The expectations come from the accepted `Bounded Jobs And Cache` and
//! `Effective Resource Accounting` clauses in `docs/specs/storage.md`, the
//! `Preparatory Cooperative Jobs` guide in `crates/knowledge-core/README.md`,
//! and the rendered public Rustdoc for `knowledge_core::jobs`.
//! The tests do not read or pin producer internals.
//!
//! The subject is the real in-process queue and reservation account. There is
//! no fake executor: a claim is settled only through the public settlement
//! operation, and deliberate abandonment remains explicitly unsettled.

use std::time::{Duration, Instant};

use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{
    Cancellation, Claim, JobError, Key, Limits, Outcome, Priority, Queue, Reservation, State,
    Submission,
};

/// Build one claimed lowercase SHA-256 value from a repeated hexadecimal digit.
fn digest(digit: char) -> ContentDigest {
    ContentDigest::new(digit.to_string().repeat(64)).expect("lowercase digest is valid")
}

/// The all-zero logical reservation used when a test isolates other controls.
fn no_reservation() -> Reservation {
    Reservation {
        memory_bytes: 0,
        temporary_bytes: 0,
    }
}

/// Owns the borrowed facts of one work key so tests can build fresh `Key` values.
#[derive(Clone)]
struct Facts {
    context: String,
    library: String,
    mount: Option<String>,
    source: String,
    version: ContentDigest,
    recipe: ContentDigest,
    processor: ContentDigest,
}

impl Facts {
    /// Create facts for one context/library/mount/source subject with fixed digests.
    fn new(context: &str, library: &str, mount: Option<&str>, source: &str) -> Self {
        Self {
            context: context.to_owned(),
            library: library.to_owned(),
            mount: mount.map(str::to_owned),
            source: source.to_owned(),
            version: digest('a'),
            recipe: digest('b'),
            processor: digest('c'),
        }
    }

    /// Replace the supplied digest facts while retaining subject identity.
    fn with_digests(mut self, version: char, recipe: char, processor: char) -> Self {
        self.version = digest(version);
        self.recipe = digest(recipe);
        self.processor = digest(processor);
        self
    }

    /// Borrow the facts as the public work identity type.
    fn key(&self) -> Key<'_> {
        Key {
            context: &self.context,
            library: &self.library,
            mount: self.mount.as_deref(),
            source: &self.source,
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.processor,
        }
    }
}

/// Create one borrowed submission with normal priority and two attempts.
fn submission<'a>(facts: &'a Facts, operation_id: &'a str, job_id: &'a str) -> Submission<'a> {
    Submission {
        operation_id,
        job_id,
        key: facts.key(),
        budget: Budget {
            max_files: 1,
            max_bytes: 0,
            max_millis: 1_000,
            max_memory_bytes: 0,
        },
        reservation: no_reservation(),
        priority: Priority::Normal,
        max_attempts: 2,
    }
}

/// Replace the logical resource promises on a copied submission.
fn reserving(mut s: Submission<'_>, memory_bytes: u64, temporary_bytes: u64) -> Submission<'_> {
    s.reservation = Reservation {
        memory_bytes,
        temporary_bytes,
    };
    s
}

/// Replace the scheduling priority on a copied submission.
fn priority(mut s: Submission<'_>, priority: Priority) -> Submission<'_> {
    s.priority = priority;
    s
}

/// Replace the finite attempt bound on a copied submission.
fn attempts(mut s: Submission<'_>, max_attempts: u64) -> Submission<'_> {
    s.max_attempts = max_attempts;
    s
}

/// Create explicit limits for tests that isolate one finite control.
fn limits(
    max_pending: usize,
    max_records: usize,
    max_descriptor_bytes: usize,
    per_mount: usize,
    memory_bytes: u64,
    temporary_bytes: u64,
) -> Limits {
    Limits {
        max_pending,
        max_records,
        max_descriptor_bytes,
        per_mount,
        memory_bytes,
        temporary_bytes,
    }
}

/// Create generous limits that keep one named control from confounding another.
fn ample_limits() -> Limits {
    limits(8, 16, 1 << 16, 8, 4_096, 4_096)
}

/// Settle one claimed job with a matching key and require the reported state.
fn settle_state(queue: &Queue, claim: &Claim, facts: &Facts, outcome: Outcome) -> State {
    queue
        .settle(claim, facts.key(), outcome)
        .expect("matching claim settles")
}

/// Settle one claimed job and require the finished terminal state.
fn settle_finished(queue: &Queue, claim: &Claim, facts: &Facts, outcome: Outcome) -> State {
    let state = settle_state(queue, claim, facts, outcome);
    assert_eq!(state, State::Finished(outcome));
    state
}

// A pending bound above the retained-record bound is inconsistent and refused.
#[test]
fn new_rejects_pending_above_record_capacity_and_accepts_equal_bounds() {
    assert!(matches!(
        Queue::new(limits(5, 4, 1 << 16, 8, 4_096, 4_096)),
        Err(JobError::Invalid)
    ));

    let queue = Queue::new(limits(4, 4, 1 << 16, 8, 4_096, 4_096)).expect("consistent bounds");
    let usage = queue.usage().expect("usable queue");
    assert_eq!(usage.jobs, 0);
    assert_eq!(usage.records, 0);
    assert_eq!(usage.pending, 0);
    assert_eq!(usage.unsettled, 0);
    assert_eq!(usage.waiting_offline, 0);
    assert_eq!(usage.descriptor_bytes, 0);
    assert_eq!(usage.reserved, no_reservation());
}

// A zero capacity closes admission and never means unlimited.
#[test]
fn zero_capacities_close_admission_and_never_mean_unlimited() {
    let facts = Facts::new("ctx", "lib", None, "source");

    let queue = Queue::new(limits(0, 0, 1 << 16, 8, 0, 0)).expect("zero bounds are valid");
    assert_eq!(
        queue.submit(submission(&facts, "op", "job")),
        Err(JobError::Capacity)
    );
    let usage = queue.usage().expect("usable queue");
    assert_eq!(usage.jobs, 0);
    assert_eq!(usage.records, 0);
    assert_eq!(usage.pending, 0);
    assert_eq!(usage.descriptor_bytes, 0);
    assert_eq!(usage.reserved, no_reservation());

    let text_closed = Queue::new(limits(1, 1, 0, 8, 4_096, 4_096)).expect("text bound valid");
    assert_eq!(
        text_closed.submit(submission(&facts, "op", "job")),
        Err(JobError::Capacity)
    );

    let memory_closed = Queue::new(limits(2, 2, 1 << 16, 8, 0, 4_096)).expect("memory bound valid");
    assert_eq!(
        memory_closed.submit(reserving(submission(&facts, "op", "job"), 1, 0)),
        Err(JobError::Capacity),
        "an impossible per-job memory promise is refused"
    );

    let temporary_closed =
        Queue::new(limits(2, 2, 1 << 16, 8, 4_096, 0)).expect("temporary bound valid");
    assert_eq!(
        temporary_closed.submit(reserving(submission(&facts, "op", "job"), 0, 1)),
        Err(JobError::Capacity),
        "an impossible per-job temporary promise is refused"
    );

    let mount_closed =
        Queue::new(limits(2, 2, 1 << 16, 0, 4_096, 4_096)).expect("mount bound valid");
    mount_closed
        .submit(submission(&facts, "op", "job"))
        .expect("admission itself is open");
    assert!(
        mount_closed
            .claim(Instant::now())
            .expect("usable queue")
            .is_none(),
        "a zero per-mount bound admits no claim"
    );
    assert_eq!(mount_closed.usage().expect("usable queue").pending, 1);
}

// Identical delivery is a no-op, new aliases share one job, and changed intent conflicts.
#[test]
fn retained_identity_reuse_is_idempotent_or_conflicting() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let a = Facts::new("ctx", "lib", Some("m1"), "a");
    let b = Facts::new("ctx", "lib", Some("m1"), "b");

    let first = queue
        .submit(submission(&a, "op", "job"))
        .expect("first delivery");
    assert!(first.created);
    let records_after_first = queue.usage().expect("usable").records;

    let identical = queue
        .submit(submission(&a, "op", "job"))
        .expect("identical delivery is a no-op");
    assert!(!identical.created);
    assert_eq!(identical.job, first.job);
    assert_eq!(
        queue.usage().expect("usable").records,
        records_after_first,
        "identical delivery consumes no new receipt"
    );

    let alias = queue
        .submit(submission(&a, "op-alias", "job-alias"))
        .expect("same key under a new operation is an alias");
    assert!(!alias.created);
    assert_eq!(alias.job, first.job);
    assert!(
        queue.usage().expect("usable").records > records_after_first,
        "each new alias keeps an explicit receipt"
    );

    assert_eq!(
        queue.submit(priority(submission(&a, "op", "job"), Priority::Pinned)),
        Err(JobError::Conflict),
        "retained operation cannot change admitted intent"
    );
    assert_eq!(
        queue.submit(submission(&b, "op", "job")),
        Err(JobError::Conflict),
        "retained operation cannot change its key"
    );
}

// The pending bound is separate from receipt capacity and frees on claim.
#[test]
fn pending_capacity_is_separate_and_frees_on_claim() {
    let queue = Queue::new(limits(2, 8, 1 << 16, 8, 4_096, 4_096)).expect("bounds valid");
    let a = Facts::new("ctx", "lib", Some("m1"), "a");
    let b = Facts::new("ctx", "lib", Some("m1"), "b");
    let c = Facts::new("ctx", "lib", Some("m1"), "c");
    let now = Instant::now();

    let sa = queue
        .submit(submission(&a, "op-a", "job-a"))
        .expect("first pending slot");
    let sb = queue
        .submit(submission(&b, "op-b", "job-b"))
        .expect("second pending slot");
    assert!(sa.created && sb.created);
    assert_eq!(
        queue.submit(submission(&c, "op-c", "job-c")),
        Err(JobError::Capacity)
    );

    let usage = queue.usage().expect("usable queue");
    assert_eq!(usage.jobs, 2);
    assert_eq!(usage.pending, 2);
    assert_eq!(usage.unsettled, 0);

    let first = queue
        .claim(now)
        .expect("usable queue")
        .expect("first job is eligible");
    assert!(first.job() == sa.job || first.job() == sb.job);
    let usage = queue.usage().expect("usable queue");
    assert_eq!(usage.pending, 1);
    assert_eq!(usage.unsettled, 1);

    let sc = queue
        .submit(submission(&c, "op-c", "job-c"))
        .expect("claim released one pending slot");
    assert!(sc.created);
    assert_eq!(queue.usage().expect("usable queue").pending, 2);

    let first_facts = if first.job() == sa.job { &a } else { &b };
    settle_finished(&queue, &first, first_facts, Outcome::Completed);

    let second = queue.claim(now).expect("usable").expect("queued job");
    let second_facts = if second.job() == sb.job { &b } else { &c };
    settle_finished(&queue, &second, second_facts, Outcome::Completed);
    let third = queue.claim(now).expect("usable").expect("remaining job");
    let third_facts = if third.job() == sc.job { &c } else { &b };
    settle_finished(&queue, &third, third_facts, Outcome::Completed);
}

// Alias receipts stay bounded, and explicit retirement forgets the whole group.
#[test]
fn alias_capacity_is_bounded_and_retirement_forgets_every_alias() {
    let queue = Queue::new(limits(3, 3, 1 << 16, 8, 4_096, 4_096)).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let base = queue
        .submit(submission(&facts, "op-0", "job-0"))
        .expect("base submission");
    assert!(base.created);

    let mut alias_count = 0;
    let mut bounded = false;
    for index in 1..=6u8 {
        let operation = format!("op-{index}");
        let job = format!("job-{index}");
        match queue.submit(submission(&facts, &operation, &job)) {
            Ok(submitted) => {
                assert_eq!(submitted.job, base.job);
                assert!(!submitted.created);
                alias_count += 1;
            }
            Err(JobError::Capacity) => {
                bounded = true;
                break;
            }
            Err(error) => panic!("unexpected alias error: {error:?}"),
        }
    }
    assert!(alias_count > 0, "at least one receipt alias fits");
    assert!(bounded, "receipt capacity bounds retained aliases");
    assert_eq!(queue.usage().expect("usable").jobs, 1);

    let now = Instant::now();
    let claim = queue.claim(now).expect("usable").expect("queued job");
    assert_eq!(queue.usage().expect("usable").pending, 0);

    let other = Facts::new("ctx", "lib", Some("m1"), "other-source");
    assert_eq!(
        queue.submit(submission(&other, "op-new", "job-new")),
        Err(JobError::Capacity),
        "receipt capacity still bounds a new canonical job"
    );

    assert_eq!(
        settle_finished(&queue, &claim, &facts, Outcome::Partial),
        State::Finished(Outcome::Partial)
    );
    queue
        .retire(base.job, facts.key())
        .expect("terminal group retires");
    let usage = queue.usage().expect("usable");
    assert_eq!(usage.jobs, 0);
    assert_eq!(usage.records, 0);
    assert_eq!(usage.pending, 0);
    assert_eq!(usage.descriptor_bytes, 0);

    let fresh = queue
        .submit(submission(&other, "op-new", "job-new"))
        .expect("retired aliases no longer consume capacity");
    assert!(fresh.created);
    assert_ne!(fresh.job, base.job, "retired job generation is not reused");
}

// Oversized borrowed metadata is refused without additional retained queue text.
#[test]
fn descriptor_capacity_refuses_oversized_metadata_without_retaining_text() {
    let facts = Facts::new("ctx", "lib", None, "source");
    let closed = Queue::new(limits(1, 1, 64, 4, 4_096, 4_096)).expect("bounds valid");
    assert_eq!(
        closed.submit(submission(&facts, "op", "job")),
        Err(JobError::Capacity),
        "mandatory identity and digest text exceeds 64 bytes"
    );
    let usage = closed.usage().expect("usable");
    assert_eq!(usage.descriptor_bytes, 0);
    assert_eq!(usage.records, 0);

    let open = Queue::new(limits(1, 1, 4_096, 4, 4_096, 4_096)).expect("bounds valid");
    let accepted = open
        .submit(submission(&facts, "op-ok", "job-ok"))
        .expect("small metadata fits");
    assert!(accepted.created);
    let usage = open.usage().expect("usable");
    assert!(
        usage.descriptor_bytes > 0,
        "accepted text is actually retained"
    );
    assert!(usage.descriptor_bytes <= 4_096);

    let before = usage.descriptor_bytes;
    let caller_owned = "x".repeat(8_192);
    assert_eq!(
        open.submit(submission(&facts, &caller_owned, "job-big")),
        Err(JobError::Capacity)
    );
    let usage = open.usage().expect("usable");
    assert_eq!(
        usage.descriptor_bytes, before,
        "refusal retained no additional queue-owned text"
    );
    assert_eq!(usage.records, 1, "refusal did not retain another receipt");
    assert_eq!(
        caller_owned.len(),
        8_192,
        "caller fixture remains owned by caller"
    );
}

// Per-mount slots are scoped by context, library and mount and bypass blocked work.
#[test]
fn per_mount_slots_are_composite_and_eligible_jobs_bypass_blocked_work() {
    let queue = Queue::new(limits(8, 8, 1 << 16, 1, 4_096, 4_096)).expect("bounds valid");
    let a = Facts::new("c1", "l1", Some("m1"), "a");
    let b = Facts::new("c1", "l2", Some("m1"), "b");
    let c = Facts::new("c1", "l1", Some("m1"), "c");
    let d = Facts::new("c1", "l1", Some("m2"), "d");
    let now = Instant::now();

    let sa = queue
        .submit(submission(&a, "op-a", "job-a"))
        .expect("a admitted");
    let sb = queue
        .submit(submission(&b, "op-b", "job-b"))
        .expect("b admitted");
    let sc = queue
        .submit(submission(&c, "op-c", "job-c"))
        .expect("c admitted");
    let sd = queue
        .submit(submission(&d, "op-d", "job-d"))
        .expect("d admitted");

    let first = queue.claim(now).expect("usable").expect("a eligible");
    assert_eq!(first.job(), sa.job);
    let second = queue
        .claim(now)
        .expect("usable")
        .expect("same mount name in another library is a distinct slot");
    assert_eq!(second.job(), sb.job);
    let third = queue
        .claim(now)
        .expect("usable")
        .expect("different mount bypasses the blocked earlier job");
    assert_eq!(third.job(), sd.job);
    assert!(
        queue.claim(now).expect("usable").is_none(),
        "c stays queued while its composite mount slot is held"
    );

    settle_finished(&queue, &first, &a, Outcome::Completed);
    settle_finished(&queue, &second, &b, Outcome::Completed);
    settle_finished(&queue, &third, &d, Outcome::Completed);

    let fourth = queue
        .claim(now)
        .expect("usable")
        .expect("freed slot admits c");
    assert_eq!(fourth.job(), sc.job);
    settle_finished(&queue, &fourth, &c, Outcome::Completed);
}

// Pinned-before-normal, then original admission order among eligible ties.
#[test]
fn claim_orders_pinned_before_normal_and_fifo_within_priority() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let a = Facts::new("ctx", "lib", Some("m1"), "a");
    let b = Facts::new("ctx", "lib", Some("m1"), "b");
    let c = Facts::new("ctx", "lib", Some("m1"), "c");
    let now = Instant::now();

    let sa = queue
        .submit(submission(&a, "op-a", "job-a"))
        .expect("a admitted");
    let sb = queue
        .submit(submission(&b, "op-b", "job-b"))
        .expect("b admitted");
    let sc = queue
        .submit(priority(submission(&c, "op-c", "job-c"), Priority::Pinned))
        .expect("c admitted pinned");

    let first = queue.claim(now).expect("usable").expect("pinned work");
    assert_eq!(first.job(), sc.job, "pinned work outranks normal work");
    let second = queue.claim(now).expect("usable").expect("normal work");
    assert_eq!(second.job(), sa.job, "normal ties keep admission order");
    let third = queue.claim(now).expect("usable").expect("normal work");
    assert_eq!(third.job(), sb.job);
    assert!(queue.claim(now).expect("usable").is_none());
}

// Cancellation and deadlines are cooperative requests that release nothing.
#[test]
fn cancellation_and_deadline_requests_hold_reservations_until_settlement() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let a = Facts::new("ctx", "lib", Some("m1"), "a");
    let b = Facts::new("ctx", "lib", Some("m1"), "b");
    let now = Instant::now();

    let sa = queue
        .submit(reserving(submission(&a, "op-a", "job-a"), 128, 64))
        .expect("a admitted");
    let sb = queue
        .submit(reserving(submission(&b, "op-b", "job-b"), 128, 64))
        .expect("b admitted");

    let claim_a = queue.claim(now).expect("usable").expect("a claimed");
    assert_eq!(claim_a.job(), sa.job);
    let claim_b = queue.claim(now).expect("usable").expect("b claimed");
    assert_eq!(claim_b.job(), sb.job);
    assert_eq!(
        queue.usage().expect("usable").reserved,
        Reservation {
            memory_bytes: 256,
            temporary_bytes: 128,
        }
    );

    assert_eq!(
        queue
            .cancel(sa.job, a.key())
            .expect("context-matched cancel"),
        State::Unsettled {
            cancellation: Some(Cancellation::Requested),
        }
    );
    assert_eq!(
        queue
            .cancel(sa.job, a.key())
            .expect("repeat cancel is idempotent"),
        State::Unsettled {
            cancellation: Some(Cancellation::Requested),
        },
        "the first cancellation reason stays recorded"
    );
    assert_eq!(
        queue.usage().expect("usable").reserved,
        Reservation {
            memory_bytes: 256,
            temporary_bytes: 128,
        },
        "a stop request releases nothing"
    );

    assert_eq!(
        queue.request_expired(now).expect("usable"),
        0,
        "no deadline has elapsed"
    );
    assert_eq!(
        queue
            .request_expired(now + Duration::from_secs(2))
            .expect("usable"),
        1,
        "only b records its first elapsed-deadline request"
    );
    let snapshot = queue.status(sb.job, b.key()).expect("retained b");
    assert_eq!(
        snapshot.state,
        State::Unsettled {
            cancellation: Some(Cancellation::Deadline),
        }
    );

    settle_finished(&queue, &claim_a, &a, Outcome::Cancelled);
    assert_eq!(
        queue.usage().expect("usable").reserved,
        Reservation {
            memory_bytes: 128,
            temporary_bytes: 64,
        }
    );
    settle_finished(&queue, &claim_b, &b, Outcome::Cancelled);
    assert_eq!(queue.usage().expect("usable").reserved, no_reservation());
}

// Offline work waits for an explicit reconnect, bounded by finite attempts.
#[test]
fn offline_settlement_waits_for_explicit_bounded_reconnect() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let now = Instant::now();

    let submitted = queue
        .submit(reserving(
            attempts(submission(&facts, "op", "job"), 2),
            200,
            100,
        ))
        .expect("work admitted");
    assert_eq!(
        queue
            .reconnect(submitted.job, facts.key())
            .expect("queued reconnect is a no-op"),
        ()
    );

    let first = queue.claim(now).expect("usable").expect("claimed once");
    assert_eq!(
        settle_state(&queue, &first, &facts, Outcome::Offline),
        State::WaitingOffline
    );
    let usage = queue.usage().expect("usable");
    assert_eq!(usage.pending, 0);
    assert_eq!(usage.unsettled, 0);
    assert_eq!(usage.waiting_offline, 1);
    assert_eq!(
        usage.reserved,
        no_reservation(),
        "the first offline settlement released its charges"
    );

    queue
        .reconnect(submitted.job, facts.key())
        .expect("one remaining attempt may reconnect");
    assert_eq!(queue.usage().expect("usable").pending, 1);

    let second = queue.claim(now).expect("usable").expect("reconnected job");
    assert_eq!(
        queue.usage().expect("usable").reserved,
        Reservation {
            memory_bytes: 200,
            temporary_bytes: 100,
        },
        "the reconnected claim holds its promise"
    );
    assert_eq!(
        settle_state(&queue, &second, &facts, Outcome::Offline),
        State::Finished(Outcome::Offline),
        "the final permitted attempt is terminal"
    );
    let snapshot = queue.status(submitted.job, facts.key()).expect("retained");
    assert_eq!(snapshot.state, State::Finished(Outcome::Offline));
    assert_eq!(snapshot.attempts, 2, "attempts are counted per claim");
    let usage = queue.usage().expect("usable");
    assert_eq!(usage.pending, 0);
    assert_eq!(usage.unsettled, 0);
    assert_eq!(
        usage.waiting_offline, 0,
        "exhausted offline work is not waiting"
    );
    assert_eq!(
        usage.reserved,
        no_reservation(),
        "final settlement released the charged reservation"
    );

    assert!(
        queue.reconnect(submitted.job, facts.key()).is_err(),
        "reconnect cannot exceed the finite attempt bound"
    );
    assert_eq!(
        queue.usage().expect("usable"),
        usage,
        "a refused reconnect changes no accounting"
    );
    let snapshot_after = queue.status(submitted.job, facts.key()).expect("retained");
    assert_eq!(snapshot_after.state, snapshot.state);
    assert_eq!(snapshot_after.attempts, snapshot.attempts);
    assert_eq!(snapshot_after.reservation, snapshot.reservation);
    assert_eq!(snapshot_after.deadline, snapshot.deadline);
}

// Invalid models are terminal and cannot be retried through reconnect.
#[test]
fn invalid_model_settlement_is_terminal_for_reconnect() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let submitted = queue
        .submit(submission(&facts, "op", "job"))
        .expect("work admitted");
    let claim = queue
        .claim(Instant::now())
        .expect("usable")
        .expect("claimed");
    assert_eq!(
        settle_finished(&queue, &claim, &facts, Outcome::InvalidModel),
        State::Finished(Outcome::InvalidModel)
    );
    assert_eq!(
        queue.reconnect(submitted.job, facts.key()),
        Err(JobError::State),
        "invalid models are terminal for reconnect"
    );
}

// A claim from another owner, a wrong context, or a settled claim releases nothing.
#[test]
fn wrong_instance_stale_and_context_claims_release_nothing_twice() {
    let first_owner = Queue::new(ample_limits()).expect("bounds valid");
    let second_owner = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let elsewhere = Facts::new("other-ctx", "lib", Some("m1"), "source");
    let submitted = first_owner
        .submit(reserving(submission(&facts, "op", "job"), 200, 100))
        .expect("work admitted");
    let claim = first_owner
        .claim(Instant::now())
        .expect("usable")
        .expect("claimed");
    assert_eq!(claim.job(), submitted.job);

    assert_eq!(
        second_owner.settle(&claim, facts.key(), Outcome::Completed),
        Err(JobError::WrongInstance),
        "another owner instance cannot release this reservation"
    );
    assert_eq!(
        first_owner.settle(&claim, elsewhere.key(), Outcome::Completed),
        Err(JobError::Context),
        "a mismatched context cannot release this reservation"
    );
    assert_eq!(
        first_owner.usage().expect("usable").unsettled,
        1,
        "refusals released nothing"
    );

    assert_eq!(
        settle_finished(&first_owner, &claim, &facts, Outcome::Completed),
        State::Finished(Outcome::Completed)
    );
    assert_eq!(
        first_owner.settle(&claim, facts.key(), Outcome::Completed),
        Err(JobError::StaleClaim),
        "a settled claim cannot release resources twice"
    );
}

// Dropping a claim performs no settlement and deliberately strands capacity.
#[test]
fn dropped_claim_strands_its_reservation_until_another_settlement() {
    let queue = Queue::new(limits(4, 4, 1 << 16, 8, 1_024, 1_024)).expect("bounds valid");
    let a = Facts::new("ctx", "lib", Some("m1"), "a");
    let b = Facts::new("ctx", "lib", Some("m1"), "b");

    queue
        .submit(reserving(submission(&a, "op-a", "job-a"), 768, 0))
        .expect("a admitted with a legal per-job promise");
    queue
        .submit(reserving(submission(&b, "op-b", "job-b"), 768, 0))
        .expect("b admitted with a legal per-job promise");

    let first = queue
        .claim(Instant::now())
        .expect("usable")
        .expect("a claimed");
    assert_eq!(queue.usage().expect("usable").unsettled, 1);
    drop(first);

    let usage = queue.usage().expect("usable");
    assert_eq!(usage.unsettled, 1, "claim drop performed no settlement");
    assert_eq!(
        usage.reserved,
        Reservation {
            memory_bytes: 768,
            temporary_bytes: 0,
        },
        "the stranded promise remains held"
    );
    assert!(
        queue.claim(Instant::now()).expect("usable").is_none(),
        "the stranded promise prevents b from taking its promise"
    );
}

// Missing jobs are unknown, mismatched facts are contextual, and retirement is bounded.
#[test]
fn status_and_retirement_distinguish_unknown_context_and_state() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let elsewhere = Facts::new("other-ctx", "lib", Some("m1"), "source");

    assert!(
        matches!(queue.status(999, facts.key()), Err(JobError::Unknown)),
        "an absent job is unknown"
    );
    assert_eq!(queue.cancel(999, facts.key()), Err(JobError::Unknown));
    assert_eq!(queue.retire(999, facts.key()), Err(JobError::Unknown));

    let submitted = queue
        .submit(submission(&facts, "op", "job"))
        .expect("work admitted");
    assert!(
        matches!(
            queue.status(submitted.job, elsewhere.key()),
            Err(JobError::Context)
        ),
        "different facts are contextual"
    );
    let snapshot = queue
        .status(submitted.job, facts.key())
        .expect("matching facts");
    assert_eq!(snapshot.state, State::Queued);
    assert_eq!(snapshot.attempts, 0);
    assert_eq!(snapshot.deadline, None);

    let claim = queue
        .claim(Instant::now())
        .expect("usable")
        .expect("claimed");
    assert_eq!(
        queue.retire(submitted.job, facts.key()),
        Err(JobError::State),
        "active work cannot be retired"
    );
    settle_finished(&queue, &claim, &facts, Outcome::Completed);
    queue
        .retire(submitted.job, facts.key())
        .expect("terminal work retires");
    assert_eq!(
        queue.retire(submitted.job, facts.key()),
        Err(JobError::Unknown),
        "retirement ends the retained idempotency window"
    );
}

// Empty identities and a zero attempt bound are invalid before admission.
#[test]
fn invalid_identities_and_zero_attempts_are_refused() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");

    assert_eq!(
        queue.submit(submission(&facts, "", "job")),
        Err(JobError::Invalid),
        "empty operation identity is invalid"
    );
    assert_eq!(
        queue.submit(submission(&facts, "op", "")),
        Err(JobError::Invalid),
        "empty job identity is invalid"
    );
    assert_eq!(
        queue.submit(attempts(submission(&facts, "op", "job"), 0)),
        Err(JobError::Invalid),
        "zero attempts is invalid"
    );
    assert_eq!(queue.usage().expect("usable").jobs, 0);
    assert_eq!(queue.usage().expect("usable").records, 0);
}

// Supersession requires the same subject and a different full key.
#[test]
fn supersession_requires_same_subject_and_different_key() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let replacement = facts.clone().with_digests('d', 'e', 'f');
    let different_source = Facts::new("ctx", "lib", Some("m1"), "other-source");

    let submitted = queue
        .submit(submission(&facts, "op", "job"))
        .expect("work admitted");

    assert_eq!(
        queue.supersede(submitted.job, facts.key(), facts.key()),
        Err(JobError::Context),
        "the same full key is not a replacement"
    );
    assert_eq!(
        queue.supersede(submitted.job, facts.key(), different_source.key()),
        Err(JobError::Context),
        "a different source is a different subject"
    );

    let state = queue
        .supersede(submitted.job, facts.key(), replacement.key())
        .expect("same subject with a different full key supersedes");
    assert!(
        matches!(
            state,
            State::Finished(Outcome::Superseded)
                | State::Unsettled {
                    cancellation: Some(Cancellation::Superseded),
                }
        ),
        "supersession is reported as either an immediate terminal result or a first stop request"
    );
}

// Structurally valid submissions keep the exact admitted work and promise facts.
#[test]
fn admitted_claim_reports_the_unchanged_budget_reservation_and_deadline() {
    let queue = Queue::new(ample_limits()).expect("bounds valid");
    let facts = Facts::new("ctx", "lib", Some("m1"), "source");
    let mut admitted = submission(&facts, "op", "job");
    admitted.budget = Budget {
        max_files: 2,
        max_bytes: 4,
        max_millis: 2_500,
        max_memory_bytes: 8,
    };
    admitted.reservation = Reservation {
        memory_bytes: 64,
        temporary_bytes: 32,
    };
    let submitted = queue.submit(admitted).expect("work admitted");

    let claim = queue
        .claim(Instant::now())
        .expect("usable")
        .expect("claimed");
    assert_eq!(claim.job(), submitted.job);
    assert_eq!(
        claim.budget(),
        admitted.budget,
        "the claim reports the unchanged admitted work maxima"
    );
    let snapshot = queue.status(submitted.job, facts.key()).expect("retained");
    assert_eq!(snapshot.budget, admitted.budget);
    assert_eq!(snapshot.reservation, admitted.reservation);
    assert!(
        snapshot.deadline.is_some(),
        "unsettled work exposes its deadline"
    );
}
