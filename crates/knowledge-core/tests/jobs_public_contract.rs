//! Public contract for the volatile cooperative job queue.
//!
//! Expectations come from the storage Bounded Jobs rule, the service job and
//! budget boundaries the queue deliberately does not execute, and the public
//! Core jobs guide plus rendered Rustdoc. No processor, backend, or worker is
//! started. Reservation counters are logical promises, not RAM or disk caps.

#[path = "jobs_public_contract/support.rs"]
mod support;

use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use knowledge_core::cache::ProcessorIdentity;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Cancellation, JobError, Outcome, Priority, Queue, State};

use support::{
    CANARY, Ident, allocated_during, assert_input_free, budget, digest, now, open_limits, queue,
    reservation, usage, warmup_window,
};

use std::alloc::{GlobalAlloc, Layout, System};

struct ThreadWindowAlloc;

unsafe impl GlobalAlloc for ThreadWindowAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            support::note_allocation(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            support::note_allocation(layout.size());
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        if !next.is_null() && new_size > layout.size() {
            support::note_allocation(new_size - layout.size());
        }
        next
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static THREAD_WINDOW_ALLOC: ThreadWindowAlloc = ThreadWindowAlloc;

/// A thread-local System window admits a real allocation and stays quiet otherwise.
#[test]
fn allocation_window_admits_a_real_copy_and_stays_quiet() {
    warmup_window();
    let (quiet_value, quiet_bytes) = allocated_during(|| black_box(0x51u64.wrapping_mul(3)));
    assert_eq!(quiet_value, 0xf3);
    assert_eq!(
        quiet_bytes, 0,
        "window counted bytes without a System request"
    );

    let (_kept, probed) = allocated_during(|| black_box(vec![7u8; 8_192]));
    assert!(
        probed >= 8_192,
        "window missed a caller-owned {probed}-byte probe"
    );
}

/// Refused borrowed metadata is not copied into the queue, while an admitted copy is.
#[test]
fn descriptor_capacity_refuses_before_queue_owned_copy() {
    warmup_window();
    let large = "m".repeat(128 * 1024);
    let mut limits = open_limits();
    limits.max_descriptor_bytes = 512;
    let blocked = queue(limits);
    let mut ident = Ident::new("copy-refuse");
    ident.operation_id = large.clone();
    let before = usage(&blocked);
    let (refused, refused_bytes) = allocated_during(|| blocked.submit(ident.submission()));
    match refused {
        Err(err) => {
            assert_eq!(err, JobError::Capacity);
            assert_input_free(&err, &large);
        }
        Ok(value) => panic!("oversized descriptor was admitted: {value:?}"),
    }
    assert_eq!(usage(&blocked).descriptor_bytes, before.descriptor_bytes);
    assert!(
        refused_bytes < large.len(),
        "refusal allocated {refused_bytes} bytes while copying {} borrowed bytes",
        large.len()
    );

    let mut accepted_limits = open_limits();
    accepted_limits.max_descriptor_bytes = 256 * 1024;
    let accepted = queue(accepted_limits);
    let mut copied = Ident::new("copy-accept");
    copied.operation_id = "c".repeat(4_096);
    let (admitted, admitted_bytes) = allocated_during(|| accepted.submit(copied.submission()));
    assert!(admitted.expect("admitted copy").created);
    assert!(
        admitted_bytes >= copied.operation_id.len(),
        "admitted copy allocated only {admitted_bytes} bytes"
    );
    assert!(usage(&accepted).descriptor_bytes >= copied.operation_id.len());
}

/// Zero closes one named capacity, and pending cannot exceed retained records.
#[test]
fn zero_capacity_closes_admission_without_becoming_unlimited() {
    let mut inverted = open_limits();
    inverted.max_pending = 4;
    inverted.max_records = 1;
    match Queue::new(inverted) {
        Err(JobError::Invalid) => {}
        Err(err) => panic!("inverted limits returned {err:?}"),
        Ok(_) => panic!("pending capacity above retained records was accepted"),
    }

    let mut closed_pending = open_limits();
    closed_pending.max_pending = 0;
    let pending = queue(closed_pending);
    let err = pending
        .submit(Ident::new("closed-pending").submission())
        .unwrap_err();
    assert_eq!(err, JobError::Capacity);
    assert_eq!(usage(&pending).jobs, 0);

    let mut closed_memory = open_limits();
    closed_memory.memory_bytes = 0;
    let memory = queue(closed_memory);
    let mut huge = Ident::new("closed-memory");
    huge.reservation = reservation(1, 0);
    assert_eq!(
        memory.submit(huge.submission()).unwrap_err(),
        JobError::Capacity
    );
    assert_eq!(usage(&memory).reserved.memory_bytes, 0);

    let mut closed_temp = open_limits();
    closed_temp.temporary_bytes = 0;
    let temp = queue(closed_temp);
    let mut wide = Ident::new("closed-temp");
    wide.reservation = reservation(0, 1);
    assert_eq!(
        temp.submit(wide.submission()).unwrap_err(),
        JobError::Capacity
    );

    let mut one_pending = open_limits();
    one_pending.max_pending = 1;
    one_pending.max_records = 4;
    let finite = queue(one_pending);
    let first = Ident::new("pending-a");
    assert!(finite.submit(first.submission()).unwrap().created);
    let second = Ident::new("pending-b");
    second_conflict_free(&finite, &second);
    assert_eq!(usage(&finite).pending, 1);
    assert_eq!(usage(&finite).records, 1);
    assert!(!finite.submit(first.submission()).unwrap().created);
    assert_eq!(
        usage(&finite).records,
        1,
        "identical delivery needs no record"
    );

    let mut empty = Ident::new("empty-id");
    empty.operation_id.clear();
    assert_eq!(
        finite.submit(empty.submission()).unwrap_err(),
        JobError::Invalid
    );
    let mut zero_attempts = Ident::new("zero-attempts");
    zero_attempts.max_attempts = 0;
    assert_eq!(
        finite.submit(zero_attempts.submission()).unwrap_err(),
        JobError::Invalid
    );
    assert_eq!(usage(&finite).records, 1);
}

fn second_conflict_free(queue: &Queue, ident: &Ident) {
    let err = queue.submit(ident.submission()).unwrap_err();
    assert_eq!(err, JobError::Capacity);
    assert_input_free(&err, &ident.operation_id);
}

/// Aliases, conflicts, UTF-8 bytes, and per-mount slots stay independent.
#[test]
fn aliases_conflicts_text_bytes_and_mount_slots_stay_independent() {
    let owned = queue(open_limits());
    let original = Ident::new("alias");
    let created = owned.submit(original.submission()).unwrap();
    assert!(created.created);
    let alias = original.clone_fields("alias-op", "alias-job");
    let aliased = owned.submit(alias.submission()).unwrap();
    assert!(!aliased.created);
    assert_eq!(aliased.job, created.job);
    let after_alias = usage(&owned);
    assert_eq!(after_alias.jobs, 1);
    assert_eq!(after_alias.records, 2);
    assert!(after_alias.descriptor_bytes >= original.operation_id.len() + alias.operation_id.len());

    let mut changed = alias.clone_fields("alias-op-2", "alias-job-2");
    changed.budget.max_bytes = original.budget.max_bytes + 1;
    changed.operation_id = format!("{CANARY}-conflict");
    let err = owned.submit(changed.submission()).unwrap_err();
    assert_eq!(err, JobError::Conflict);
    assert_input_free(&err, CANARY);
    assert_eq!(usage(&owned).records, 2);

    let mut full = open_limits();
    full.max_pending = 1;
    full.max_records = 1;
    let bounded = queue(full);
    let only = Ident::new("only-record");
    assert!(bounded.submit(only.submission()).unwrap().created);
    let extra = only.clone_fields("second-op", "second-job");
    assert_eq!(
        bounded.submit(extra.submission()).unwrap_err(),
        JobError::Capacity
    );
    assert_eq!(usage(&bounded).records, 1);

    let ascii = queue(open_limits());
    let mut short = Ident::new("utf");
    short.source = "aa".to_owned();
    ascii.submit(short.submission()).unwrap();
    let text = queue(open_limits());
    let mut wide = Ident::new("utf");
    wide.source = "文档".to_owned();
    text.submit(wide.submission()).unwrap();
    assert_eq!(
        usage(&text).descriptor_bytes - usage(&ascii).descriptor_bytes,
        wide.source.len() - short.source.len()
    );

    let mut slots = open_limits();
    slots.per_mount = 1;
    slots.memory_bytes = 10_000;
    let mounted = queue(slots);
    let mut home = Ident::new("home");
    home.mount = None;
    home.reservation = reservation(0, 0);
    let mut named = Ident::new("named");
    named.mount = Some("mount-a".to_owned());
    named.reservation = reservation(0, 0);
    let mut again = named.clone_fields("named-2", "named-job-2");
    again.operation_id = "named-op-2".to_owned();
    again.job_id = "named-job-2".to_owned();
    again.version = digest(0x44);
    mounted.submit(home.submission()).unwrap();
    mounted.submit(named.submission()).unwrap();
    mounted.submit(again.submission()).unwrap();
    let first = mounted.claim(now()).unwrap().expect("home or first mount");
    let second = mounted.claim(now()).unwrap().expect("other mount");
    assert_ne!(first.job(), second.job());
    assert!(
        mounted.claim(now()).unwrap().is_none(),
        "same mount stays blocked"
    );
    assert_eq!(usage(&mounted).unsettled, 2);
    assert_eq!(usage(&mounted).pending, 1);
    assert_eq!(usage(&mounted).reserved.memory_bytes, 0);
}

/// Pinned work precedes normal FIFO, and a blocked head does not stop another mount or a smaller promise.
#[test]
fn eligible_priority_fifo_bypasses_a_blocked_head() {
    let mut limits = open_limits();
    limits.per_mount = 4;
    limits.memory_bytes = 1_000;
    limits.temporary_bytes = 1_000;
    let order = queue(limits);
    let mut normal_a = Ident::new("fifo-a");
    normal_a.mount = Some("shared".to_owned());
    normal_a.reservation = reservation(1, 1);
    let mut normal_b = normal_a.clone_fields("fifo-b-op", "fifo-b-job");
    normal_b.version = digest(0x45);
    let mut pinned = normal_a.clone_fields("fifo-p-op", "fifo-p-job");
    pinned.version = digest(0x46);
    pinned.priority = Priority::Pinned;
    let a = order.submit(normal_a.submission()).unwrap().job;
    let b = order.submit(normal_b.submission()).unwrap().job;
    let p = order.submit(pinned.submission()).unwrap().job;
    let first = order.claim(now()).unwrap().expect("pinned");
    let second = order.claim(now()).unwrap().expect("earlier normal");
    let third = order.claim(now()).unwrap().expect("later normal");
    assert_eq!([first.job(), second.job(), third.job()], [p, a, b]);

    let mut blocked_limits = open_limits();
    blocked_limits.per_mount = 1;
    blocked_limits.memory_bytes = 30;
    blocked_limits.temporary_bytes = 30;
    let bypass = queue(blocked_limits);
    let mut head = Ident::new("head");
    head.mount = Some("mount-1".to_owned());
    head.reservation = reservation(8, 8);
    let mut behind = head.clone_fields("behind-op", "behind-job");
    behind.version = digest(0x47);
    let mut other = Ident::new("other");
    other.mount = Some("mount-2".to_owned());
    other.reservation = reservation(2, 2);
    let head_job = bypass.submit(head.submission()).unwrap().job;
    let behind_job = bypass.submit(behind.submission()).unwrap().job;
    let other_job = bypass.submit(other.submission()).unwrap().job;
    let claimed_head = bypass.claim(now()).unwrap().expect("head fits");
    assert_eq!(claimed_head.job(), head_job);
    let claimed_other = bypass.claim(now()).unwrap().expect("other mount bypasses");
    assert_eq!(claimed_other.job(), other_job);
    assert!(matches!(
        bypass.status(behind_job, behind.key()).unwrap().state,
        State::Queued
    ));
    assert!(bypass.claim(now()).unwrap().is_none());

    let mut tight = open_limits();
    tight.memory_bytes = 10;
    tight.temporary_bytes = 10;
    tight.per_mount = 4;
    let resources = queue(tight);
    let mut wide = Ident::new("wide");
    wide.mount = Some("only".to_owned());
    wide.reservation = reservation(8, 8);
    let mut later = wide.clone_fields("later-op", "later-job");
    later.version = digest(0x48);
    later.reservation = reservation(8, 8);
    let mut small = wide.clone_fields("small-op", "small-job");
    small.version = digest(0x49);
    small.reservation = reservation(2, 2);
    let wide_job = resources.submit(wide.submission()).unwrap().job;
    let later_job = resources.submit(later.submission()).unwrap().job;
    let small_job = resources.submit(small.submission()).unwrap().job;
    assert_eq!(
        resources.claim(now()).unwrap().expect("wide").job(),
        wide_job
    );
    assert_eq!(
        resources.claim(now()).unwrap().expect("small bypass").job(),
        small_job
    );
    assert!(matches!(
        resources.status(later_job, later.key()).unwrap().state,
        State::Queued
    ));
    assert_eq!(usage(&resources).reserved.memory_bytes, 10);
    assert_eq!(usage(&resources).reserved.temporary_bytes, 10);
    assert!(resources.claim(now()).unwrap().is_none());
}

/// Memory, temporary space, and one mount slot move together, and sums do not wrap.
#[test]
fn reservations_are_atomic_and_do_not_wrap() {
    let mut limits = open_limits();
    limits.memory_bytes = u64::MAX - 10;
    limits.temporary_bytes = u64::MAX - 10;
    limits.per_mount = 2;
    limits.max_pending = 2;
    limits.max_records = 2;
    let owned = queue(limits);
    let mut first = Ident::new("wrap-a");
    first.reservation = reservation(u64::MAX - 20, 5);
    let mut second = first.clone_fields("wrap-b-op", "wrap-b-job");
    second.version = digest(0x4a);
    second.reservation = reservation(30, 5);
    let first_job = owned.submit(first.submission()).unwrap().job;
    owned.submit(second.submission()).unwrap();
    warmup_window();
    let (claimed, claim_bytes) = allocated_during(|| owned.claim(now()));
    let claim = claimed.expect("claim").expect("first reservation fits");
    assert_eq!(claim.job(), first_job);
    assert!(
        claim_bytes < 1_048_576,
        "logical reservation materialized {claim_bytes} native bytes"
    );
    assert_eq!(usage(&owned).reserved.memory_bytes, u64::MAX - 20);
    assert!(owned.claim(now()).unwrap().is_none(), "sum would wrap");
    assert_eq!(usage(&owned).reserved.memory_bytes, u64::MAX - 20);
    assert_eq!(usage(&owned).unsettled, 1);
    assert_eq!(usage(&owned).pending, 1);

    let err = owned
        .settle(&claim, second.key(), Outcome::Completed)
        .unwrap_err();
    assert_eq!(err, JobError::Context);
    assert_eq!(usage(&owned).reserved.memory_bytes, u64::MAX - 20);
    let state = owned
        .settle(&claim, first.key(), Outcome::Completed)
        .unwrap();
    assert_eq!(state, State::Finished(Outcome::Completed));
    assert_eq!(usage(&owned).reserved.memory_bytes, 0);
    assert_eq!(usage(&owned).unsettled, 0);
    assert_eq!(
        owned
            .settle(&claim, first.key(), Outcome::Completed)
            .unwrap_err(),
        JobError::StaleClaim
    );
    assert_eq!(usage(&owned).reserved.memory_bytes, 0);
}

/// Stop requests retain the first reason and active promises; completion can still win.
#[test]
fn stop_requests_keep_promises_until_actual_settlement() {
    let owned = queue(open_limits());
    let queued = Ident::new("cancel-queued");
    let job = owned.submit(queued.submission()).unwrap().job;
    let cancelled = owned.cancel(job, queued.key()).unwrap();
    assert_eq!(cancelled, State::Finished(Outcome::Cancelled));
    assert_eq!(owned.cancel(job, queued.key()).unwrap(), cancelled);
    assert_eq!(usage(&owned).pending, 0);
    assert_eq!(usage(&owned).jobs, 1);

    let active = Ident::new("cancel-active");
    let active_job = owned.submit(active.submission()).unwrap().job;
    let claim = owned.claim(now()).unwrap().expect("active");
    assert_eq!(claim.job(), active_job);
    assert_eq!(claim.budget(), active.budget);
    let held = usage(&owned).reserved;
    let requested = owned.cancel(active_job, active.key()).unwrap();
    assert_eq!(
        requested,
        State::Unsettled {
            cancellation: Some(Cancellation::Requested)
        }
    );
    assert_eq!(usage(&owned).reserved, held);
    let mut replacement = active.clone_fields("unused-op", "unused-job");
    replacement.version = digest(0x4b);
    let after_supersede = owned
        .supersede(active_job, active.key(), replacement.key())
        .unwrap();
    assert_eq!(after_supersede, requested, "first stop reason stays");
    assert_eq!(usage(&owned).jobs, 2);
    assert_eq!(
        owned
            .settle(&claim, active.key(), Outcome::Completed)
            .unwrap(),
        State::Finished(Outcome::Completed)
    );
    assert_eq!(usage(&owned).reserved.memory_bytes, 0);

    let deadline = Ident::new("deadline");
    let deadline_job = owned.submit(deadline.submission()).unwrap().job;
    let started = now();
    let claimed = owned.claim(started).unwrap().expect("deadline claim");
    assert_eq!(claimed.job(), deadline_job);
    assert!(claimed.deadline() >= started);
    assert_eq!(owned.request_expired(started).unwrap(), 0);
    let due = started
        .checked_add(Duration::from_millis(deadline.budget.max_millis + 1))
        .expect("representable deadline");
    assert_eq!(owned.request_expired(due).unwrap(), 1);
    assert_eq!(
        owned.request_expired(due).unwrap(),
        0,
        "the first request stays"
    );
    assert_eq!(
        owned.status(deadline_job, deadline.key()).unwrap().state,
        State::Unsettled {
            cancellation: Some(Cancellation::Deadline)
        }
    );
    assert_eq!(
        usage(&owned).reserved.memory_bytes,
        deadline.reservation.memory_bytes
    );
    let mut other_version = deadline.clone_fields("deadline-next-op", "deadline-next-job");
    other_version.version = digest(0x01);
    let obsolete = owned
        .supersede(deadline_job, deadline.key(), other_version.key())
        .unwrap();
    assert_eq!(
        obsolete,
        State::Unsettled {
            cancellation: Some(Cancellation::Deadline)
        }
    );

    let fresh = Ident::new("supersede-live");
    let fresh_job = owned.submit(fresh.submission()).unwrap().job;
    let live = owned.claim(now()).unwrap().expect("live");
    assert_eq!(live.job(), fresh_job);
    let mut next_key = fresh.clone_fields("not-submitted-op", "not-submitted-job");
    next_key.version = digest(0x4c);
    let jobs_before = usage(&owned).jobs;
    let marked = owned
        .supersede(fresh_job, fresh.key(), next_key.key())
        .unwrap();
    assert_eq!(
        marked,
        State::Unsettled {
            cancellation: Some(Cancellation::Superseded)
        }
    );
    assert_eq!(usage(&owned).jobs, jobs_before);
    assert_eq!(
        owned
            .supersede(fresh_job, next_key.key(), fresh.key())
            .unwrap_err(),
        JobError::Context
    );
    owned
        .settle(&live, fresh.key(), Outcome::Superseded)
        .unwrap();

    let mut waiting = Ident::new("lexicographic");
    waiting.version = digest(0xaa);
    let mut smaller = waiting.clone_fields("smaller-op", "smaller-job");
    smaller.version = digest(0x01);
    let waiting_job = owned.submit(waiting.submission()).unwrap().job;
    let smaller_job = owned.submit(smaller.submission()).unwrap().job;
    let fifo = owned.claim(now()).unwrap().expect("fifo");
    assert_eq!(fifo.job(), waiting_job);
    let _held_earlier = fifo;
    let second = owned.claim(now()).unwrap().expect("later digest");
    assert_eq!(second.job(), smaller_job);
    owned
        .settle(&second, smaller.key(), Outcome::Completed)
        .unwrap();

    let obsolete = queue(open_limits());
    let mut old = Ident::new("obsolete");
    old.version = digest(0xaa);
    let mut replacement = old.clone_fields("not-submitted", "not-submitted-job");
    replacement.version = digest(0x01);
    let old_job = obsolete.submit(old.submission()).unwrap().job;
    assert_eq!(
        obsolete
            .supersede(old_job, old.key(), old.key())
            .unwrap_err(),
        JobError::Context
    );
    assert!(matches!(
        obsolete.status(old_job, old.key()).unwrap().state,
        State::Queued
    ));
    let retained = usage(&obsolete).jobs;
    let state = obsolete
        .supersede(old_job, old.key(), replacement.key())
        .unwrap();
    assert_eq!(usage(&obsolete).jobs, retained);
    assert_eq!(usage(&obsolete).pending, 0);
    assert_eq!(usage(&obsolete).reserved.memory_bytes, 0);
    assert!(obsolete.claim(now()).unwrap().is_none());
    assert!(!matches!(state, State::Queued | State::Unsettled { .. }));
}

/// Offline work reconnects only inside the attempt and pending bounds.
#[test]
fn offline_reconnect_is_explicit_and_other_failures_stay_terminal() {
    let mut limits = open_limits();
    limits.max_pending = 1;
    let owned = queue(limits);
    let mut retry = Ident::new("offline");
    retry.max_attempts = 2;
    let job = owned.submit(retry.submission()).unwrap().job;
    let claim = owned.claim(now()).unwrap().expect("attempt");
    assert_eq!(owned.status(job, retry.key()).unwrap().attempts, 1);
    assert_eq!(
        owned.settle(&claim, retry.key(), Outcome::Offline).unwrap(),
        State::WaitingOffline
    );
    assert_eq!(usage(&owned).reserved.memory_bytes, 0);
    assert_eq!(usage(&owned).waiting_offline, 1);
    assert_eq!(usage(&owned).pending, 0);
    assert_eq!(
        owned.status(job, retry.key()).unwrap().state,
        State::WaitingOffline
    );
    owned.reconnect(job, retry.key()).unwrap();
    assert!(matches!(
        owned.status(job, retry.key()).unwrap().state,
        State::Queued
    ));
    owned.reconnect(job, retry.key()).unwrap();
    assert_eq!(usage(&owned).records, 1);
    let again = owned.claim(now()).unwrap().expect("second attempt");
    assert_eq!(owned.status(job, retry.key()).unwrap().attempts, 2);
    owned.settle(&again, retry.key(), Outcome::Offline).unwrap();
    assert_eq!(owned.status(job, retry.key()).unwrap().attempts, 2);
    assert_eq!(usage(&owned).reserved.memory_bytes, 0);
    let observed = owned.status(job, retry.key()).unwrap().state;
    let stuck = usage(&owned);
    assert!(owned.reconnect(job, retry.key()).is_err());
    assert_eq!(usage(&owned), stuck);
    assert_eq!(owned.status(job, retry.key()).unwrap().state, observed);
    assert_eq!(owned.status(job, retry.key()).unwrap().attempts, 2);

    let mut blocked = open_limits();
    blocked.max_pending = 1;
    let pending = queue(blocked);
    let mut parked = Ident::new("parked");
    parked.max_attempts = 3;
    let parked_job = pending.submit(parked.submission()).unwrap().job;
    let parked_claim = pending.claim(now()).unwrap().expect("park");
    pending
        .settle(&parked_claim, parked.key(), Outcome::Offline)
        .unwrap();
    let filler = Ident::new("filler");
    pending.submit(filler.submission()).unwrap();
    assert_eq!(
        pending.reconnect(parked_job, parked.key()).unwrap_err(),
        JobError::Capacity
    );
    assert_eq!(
        pending.status(parked_job, parked.key()).unwrap().state,
        State::WaitingOffline
    );

    let terminal = queue(open_limits());
    for outcome in [
        Outcome::InvalidModel,
        Outcome::Failed,
        Outcome::Partial,
        Outcome::Completed,
    ] {
        let mut ident = Ident::new(&format!("terminal-{outcome:?}"));
        ident.version = digest(outcome_seed(outcome));
        let id = terminal.submit(ident.submission()).unwrap().job;
        let claim = terminal.claim(now()).unwrap().expect("terminal claim");
        assert_eq!(
            terminal.settle(&claim, ident.key(), outcome).unwrap(),
            State::Finished(outcome)
        );
        assert_eq!(
            terminal.reconnect(id, ident.key()).unwrap_err(),
            JobError::State
        );
        assert_eq!(
            terminal.cancel(id, ident.key()).unwrap(),
            State::Finished(outcome)
        );
    }
}

fn outcome_seed(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::InvalidModel => 0x61,
        Outcome::Failed => 0x62,
        Outcome::Partial => 0x63,
        Outcome::Completed => 0x64,
        Outcome::Cancelled | Outcome::Superseded | Outcome::Offline => 0x65,
    }
}

/// Retirement forgets aliases only after a terminal record, and job numbers keep advancing.
#[test]
fn retirement_forgets_terminal_aliases_and_numbers_keep_advancing() {
    let owned = queue(open_limits());
    let mut work = Ident::new("retire");
    let first = owned.submit(work.submission()).unwrap();
    let alias = work.clone_fields("retire-alias-op", "retire-alias-job");
    owned.submit(alias.submission()).unwrap();
    assert_eq!(
        owned.retire(first.job, work.key()).unwrap_err(),
        JobError::State
    );
    let claim = owned.claim(now()).unwrap().expect("retire claim");
    assert_eq!(
        owned.retire(first.job, work.key()).unwrap_err(),
        JobError::State
    );
    owned
        .settle(&claim, work.key(), Outcome::Completed)
        .unwrap();
    assert_eq!(
        owned.reconnect(first.job, work.key()).unwrap_err(),
        JobError::State
    );
    owned.retire(first.job, work.key()).unwrap();
    assert_eq!(
        owned.status(first.job, work.key()).unwrap_err(),
        JobError::Unknown
    );
    assert_eq!(usage(&owned).jobs, 0);
    assert_eq!(usage(&owned).records, 0);
    assert_eq!(usage(&owned).descriptor_bytes, 0);
    assert_eq!(
        owned.retire(first.job, work.key()).unwrap_err(),
        JobError::Unknown
    );
    work.operation_id = "retire-again".to_owned();
    let second = owned.submit(work.submission()).unwrap();
    assert!(second.created);
    assert!(second.job > first.job);
}

/// A dropped claim remains an unsettled promise, and its owner can settle after the handle drops.
#[test]
fn dropped_claim_remains_logically_unsettled() {
    let owned = queue(open_limits());
    let ident = Ident::new("drop-claim");
    let job = owned.submit(ident.submission()).unwrap().job;
    let claim = owned.claim(now()).unwrap().expect("claim");
    let reserved = usage(&owned).reserved;
    drop(claim);
    assert!(matches!(
        owned.status(job, ident.key()).unwrap().state,
        State::Unsettled { cancellation: None }
    ));
    let after_drop = usage(&owned);
    assert_eq!(after_drop.unsettled, 1);
    assert_eq!(after_drop.reserved, reserved);
    assert_eq!(after_drop.pending, 0);
    assert!(owned.claim(now()).unwrap().is_none());

    let retained = queue(open_limits());
    let live = Ident::new("owner-drop");
    let live_job = retained.submit(live.submission()).unwrap().job;
    let claim = retained.claim(now()).unwrap().expect("owned claim");
    assert_eq!(claim.job(), live_job);
    let owner = claim.owner().clone();
    drop(retained);
    assert_eq!(
        claim
            .owner()
            .settle(&claim, live.key(), Outcome::Completed)
            .unwrap(),
        State::Finished(Outcome::Completed)
    );
    assert_eq!(usage(&owner).unsettled, 0);
    assert_eq!(usage(&owner).reserved.memory_bytes, 0);
    assert_eq!(
        owner.status(live_job, live.key()).unwrap().state,
        State::Finished(Outcome::Completed)
    );
    assert_eq!(
        owner.status(u64::MAX, live.key()).unwrap_err(),
        JobError::Unknown
    );
}

/// Another instance, a stale generation, and a mismatched key cannot release a promise twice.
#[test]
fn foreign_and_stale_settlement_does_not_release_twice() {
    let owner = queue(open_limits());
    let other = queue(open_limits());
    let ident = Ident::new("instance");
    let job = owner.submit(ident.submission()).unwrap().job;
    let claim = owner.claim(now()).unwrap().expect("local claim");
    assert_eq!(claim.job(), job);
    assert_eq!(
        other
            .settle(&claim, ident.key(), Outcome::Completed)
            .unwrap_err(),
        JobError::WrongInstance
    );
    assert_eq!(usage(&owner).unsettled, 1);
    assert_eq!(usage(&other).jobs, 0);
    owner.settle(&claim, ident.key(), Outcome::Failed).unwrap();
    assert_eq!(
        owner
            .settle(&claim, ident.key(), Outcome::Failed)
            .unwrap_err(),
        JobError::StaleClaim
    );
    assert_eq!(usage(&owner).reserved.memory_bytes, 0);
    assert_eq!(usage(&owner).unsettled, 0);

    let mismatched = queue(open_limits());
    let mut wrong = Ident::new("context");
    wrong.operation_id = format!("{CANARY}-context");
    let id = mismatched.submit(wrong.submission()).unwrap().job;
    let claim = mismatched.claim(now()).unwrap().expect("context claim");
    let mut shifted = wrong.clone_fields("context-b-op", "context-b-job");
    shifted.library = format!("{CANARY}-library");
    let err = mismatched
        .settle(&claim, shifted.key(), Outcome::Completed)
        .unwrap_err();
    assert_eq!(err, JobError::Context);
    assert_input_free(&err, CANARY);
    assert_eq!(
        mismatched.status(id, shifted.key()).unwrap_err(),
        JobError::Context
    );
    mismatched
        .settle(&claim, wrong.key(), Outcome::Completed)
        .unwrap();
}

/// Queue success and a claimed digest do not touch a path or execute a budget.
#[test]
fn success_and_digest_spelling_are_not_authority() {
    assert!(ContentDigest::new("ab".repeat(32)).is_ok());
    assert!(ContentDigest::new("AB".repeat(32)).is_err());
    assert!(ContentDigest::new("a".repeat(63)).is_err());
    assert!(ContentDigest::new(format!("zz{}", "ab".repeat(31))).is_err());

    let marker = Path::new("synthetic-jobs-public-9c2e-not-created");
    assert!(!marker.exists());
    let owned = queue(open_limits());
    let mut ident = Ident::new("authority");
    ident.budget = budget(15_000);
    ident.reservation = reservation(3, 9);
    ident.processor = fingerprint_digest();
    let submitted = owned.submit(ident.submission()).unwrap();
    let claim = owned.claim(now()).unwrap().expect("claim");
    assert_eq!(claim.job(), submitted.job);
    assert_eq!(claim.budget(), ident.budget);
    assert_ne!(
        claim.budget().max_memory_bytes,
        ident.reservation.memory_bytes
    );
    assert_eq!(
        owned.status(submitted.job, ident.key()).unwrap().budget,
        ident.budget
    );
    owned
        .settle(&claim, ident.key(), Outcome::Completed)
        .unwrap();
    assert!(!marker.exists());

    let mut clock_limits = open_limits();
    clock_limits.max_pending = 1;
    let clock = queue(clock_limits);
    let mut far = Ident::new("clock");
    far.budget.max_millis = u64::MAX;
    let instant = now();
    let representable = instant.checked_add(Duration::from_millis(u64::MAX));
    clock.submit(far.submission()).unwrap();
    let before = usage(&clock);
    match (representable.is_some(), clock.claim(instant)) {
        (false, Err(JobError::Clock)) => assert_eq!(usage(&clock), before),
        (false, Err(err)) => panic!("unrepresentable deadline returned {err:?}"),
        (false, Ok(_)) => panic!("unrepresentable deadline was claimed"),
        (true, Ok(Some(claim))) => {
            assert_eq!(claim.budget().max_millis, u64::MAX);
            assert_eq!(
                usage(&clock).reserved.memory_bytes,
                far.reservation.memory_bytes
            );
            assert_eq!(usage(&clock).unsettled, 1);
        }
        (true, Ok(None)) => panic!("representable deadline was not claimed"),
        (true, Err(err)) => panic!("representable deadline returned {err:?}"),
    }
}

fn fingerprint_digest() -> ContentDigest {
    let mut first = BTreeMap::new();
    first.insert("b".to_owned(), "1".to_owned());
    first.insert("a".to_owned(), "1".to_owned());
    let mut second = BTreeMap::new();
    second.insert("a".to_owned(), "1".to_owned());
    second.insert("b".to_owned(), "1".to_owned());
    let left = identity(first).fingerprint(8_192).expect("fingerprint");
    let right = identity(second).fingerprint(8_192).expect("fingerprint");
    assert_eq!(left.as_str(), right.as_str());
    assert_eq!(left.as_str().len(), 64);
    let mut changed = BTreeMap::new();
    changed.insert("a".to_owned(), "2".to_owned());
    let other = identity(changed).fingerprint(8_192).expect("fingerprint");
    assert_ne!(left.as_str(), other.as_str());
    left
}

fn identity(options: BTreeMap<String, String>) -> ProcessorIdentity {
    ProcessorIdentity {
        code: digest(0x70),
        version: "v1".to_owned(),
        options,
        model: None,
        package: None,
        space: None,
        dimensions: None,
        preprocessing: BTreeMap::new(),
        dependencies: BTreeMap::new(),
    }
}

/// Joined caller threads keep one-winner admission, claim, and settlement.
#[test]
fn joined_callers_admit_claim_and_settle_once() {
    let mut limits = open_limits();
    limits.max_records = 1;
    limits.max_pending = 1;
    let admitted = queue(limits);
    let mut results = Vec::new();
    thread::scope(|scope| {
        let barrier = Arc::new(Barrier::new(4));
        let mut joins = Vec::new();
        for index in 0..4 {
            let admitted = admitted.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(scope.spawn(move || {
                let ident = Ident::new(&format!("race-admit-{index}"));
                barrier.wait();
                admitted.submit(ident.submission())
            }));
        }
        for join in joins {
            results.push(join.join().expect("admit thread"));
        }
    });
    let wins = results.iter().filter(|result| result.is_ok()).count();
    assert_eq!(wins, 1);
    assert_eq!(usage(&admitted).records, 1);
    assert_eq!(usage(&admitted).jobs, 1);

    let shared = queue(open_limits());
    let ident = Ident::new("race-claim");
    let job = shared.submit(ident.submission()).unwrap().job;
    let mut claims = Vec::new();
    thread::scope(|scope| {
        let barrier = Arc::new(Barrier::new(4));
        let mut joins = Vec::new();
        for _ in 0..4 {
            let shared = shared.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(scope.spawn(move || {
                barrier.wait();
                shared.claim(now())
            }));
        }
        for join in joins {
            claims.push(join.join().expect("claim thread"));
        }
    });
    let won = claims
        .into_iter()
        .filter_map(|result| result.expect("claim result"))
        .count();
    assert_eq!(won, 1);
    assert!(matches!(
        shared.status(job, ident.key()).unwrap().state,
        State::Unsettled { .. }
    ));
    assert_eq!(usage(&shared).unsettled, 1);
    assert_eq!(usage(&shared).reserved, ident.reservation);
    assert_eq!(usage(&shared).pending, 0);

    let claim = shared.claim(now()).unwrap();
    assert!(claim.is_none());
    let settled_owner = queue(open_limits());
    let subject = Ident::new("race-settle");
    let subject_job = settled_owner.submit(subject.submission()).unwrap().job;
    let claim = settled_owner.claim(now()).unwrap().expect("settle subject");
    assert_eq!(claim.job(), subject_job);
    let claim = &claim;
    let subject = &subject;
    let mut settlements = Vec::new();
    thread::scope(|scope| {
        let barrier = Arc::new(Barrier::new(4));
        let mut joins = Vec::new();
        for _ in 0..4 {
            let settled_owner = settled_owner.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(scope.spawn(move || {
                barrier.wait();
                settled_owner.settle(claim, subject.key(), Outcome::Completed)
            }));
        }
        for join in joins {
            settlements.push(join.join().expect("settle thread"));
        }
    });
    let releases = settlements.iter().filter(|result| result.is_ok()).count();
    assert_eq!(releases, 1);
    assert!(
        settlements
            .iter()
            .any(|result| matches!(result, Err(JobError::StaleClaim)))
    );
    assert_eq!(usage(&settled_owner).reserved.memory_bytes, 0);
    assert_eq!(usage(&settled_owner).unsettled, 0);
    assert_eq!(usage(&settled_owner).jobs, 1);
    let _ = job;
}

trait CloneFields {
    fn clone_fields(&self, operation_id: &str, job_id: &str) -> Self;
}

impl CloneFields for Ident {
    fn clone_fields(&self, operation_id: &str, job_id: &str) -> Self {
        Self {
            context: self.context.clone(),
            library: self.library.clone(),
            mount: self.mount.clone(),
            source: self.source.clone(),
            operation_id: operation_id.to_owned(),
            job_id: job_id.to_owned(),
            version: self.version.clone(),
            recipe: self.recipe.clone(),
            processor: self.processor.clone(),
            budget: self.budget,
            reservation: self.reservation,
            priority: self.priority,
            max_attempts: self.max_attempts,
        }
    }
}
