# Knowledge-Core Public Jobs Contract Tests

This record owns the independent integration tests for the portable cooperative job queue.
It does not accept the queue, amend storage, or start an executor.
The tests are `crates/knowledge-core/tests/jobs_public_contract.rs` and `crates/knowledge-core/tests/jobs_public_contract/support.rs`.
Production lines in this change: 0.
The lease forbids production edits, so the test-to-production ratio is not a reason to add implementation.

## Authority And Independence

Expected observations come from the Bounded Jobs And Cache and Effective Resource Accounting clauses in `docs/specs/storage.md`, the public Core guide section Preparatory Cooperative Jobs, and public Rustdoc rendered for `jobs`, `Budget`, and `ContentDigest`.
`ProcessorIdentity::fingerprint` is used only as the cache seam named by that jobs guide.
The adopted process resource policy is recorded as later runtime integration.
It is not treated as an executor, an operating-system cap, or the portable queue's default.
`docs/changes/2026-11-04-bounded-jobs.md` was not opened.
Candidate bodies, builder probes, other test pools, and prior jobs findings were not inputs.
A failing check stays a failing check.
This record does not retune an expectation to obtain a pass.
Two constructor and comparison fixtures were corrected before the recorded run: a one-record queue also had `max_pending` of 1, and the UTF-8 pair differed only in `source`.
Those corrections let the stated assertions run.
They did not change a product expectation after a mismatch.

## What The Suite Decides

`Queue::new` rejects pending capacity above retained-record capacity.
A zero pending, memory, or temporary ceiling refuses the corresponding admission with `Capacity` and does not become unlimited.
An empty operation id and a zero attempt ceiling are `Invalid`.
The same context, library, mount, and version is one job.
A second submission with the same budget and reservation is an alias, `created` is false, and both operation records remain.
A changed budget on that key is `Conflict` and does not echo the operation id.
A one-record ceiling refuses the alias with `Capacity`.
UTF-8 `source` text is counted in descriptor bytes, not Unicode scalar values.
One mount slot admits home and a named mount, then blocks a third claim while both claims stay unsettled.
Pinned work precedes normal work, and normal work stays in submission order.
A head blocked by its mount slot or by a reservation larger than the remainder does not block a later eligible job.
Memory and temporary bytes reserve together or not at all.
A reservation that would overflow `u64` is not admitted and does not allocate a native buffer of that size.
The refused claim leaves the prior reservation unchanged.
`WrongInstance`, `StaleClaim`, and `Context` do not release a live reservation twice.
A queued cancellation becomes `Finished(Cancelled)` and repeats.
An unsettled cancellation stays `Requested` until settlement, and a later supersede does not replace that reason.
`request_expired` is caller-clocked, idempotent after the first hit, and holds the reservation as `Deadline`.
`Completed` may settle a job that already has a cancellation reason.
An unsettled supersede records `Superseded` and does not submit the replacement.
A queued supersede with the same key is `Context`.
A queued supersede with a different key removes the old job from the claimable set and does not create a replacement job.
The exact terminal variant of that queued supersede is not pinned.
The first of four joined submitters wins a one-record queue.
The first of four joined claimers wins one job.
The first of four joined settlers wins that claim, and the others receive `StaleClaim`.
Ordering uses a barrier and `thread::scope` joins.
No sleep is used as an order.
Dropping a `Claim` leaves the job `Unsettled` with the reservation held.
Dropping one queue handle does not settle the shared account.
`retire` refuses a queued or unsettled job, then forgets a terminal job and its alias.
The next submission receives a greater job number.
The first offline settlement is `WaitingOffline`, releases the reservation, and returns only through explicit `reconnect`.
A pending-full reconnect is `Capacity`.
`InvalidModel`, `Failed`, `Partial`, and `Completed` stay terminal under reconnect and repeated cancellation.
A digest is 64 lowercase hex characters and is not proof that a file was hashed.
Settlement does not create the synthetic path named by the test.
A passed-through `Budget` is distinct from the queue `Reservation`.
The processor fingerprint is stable across `BTreeMap` insertion order.

## Retained Failures

`offline_reconnect_is_explicit_and_other_failures_stay_terminal` failed at the status check after the second offline settlement and the refused reconnect.
The observed state was `Finished(Offline)`.
The retained expectation is `WaitingOffline`.
The earlier assertions in that test had already passed: the first offline settlement was `WaitingOffline`, reservations were released, explicit reconnect requeued the job, the second claim observed two attempts, the following reconnect returned an error, and usage did not change on that error.
The pending-full `Capacity` case and the terminal-outcome cases in the same function did not run in the failing execution.
`success_and_digest_spelling_are_not_authority` failed when `claim` accepted `max_millis` of `u64::MAX`.
The retained expectation is `JobError::Clock` and unchanged usage.
Digest spelling, the absent path, budget pass-through, and fingerprint stability in that function had already passed.
The usage comparison after the clock refusal did not run.

## Coverage Matrix

| Owner or public clause | Test |
| --- | --- |
| Explicit finite ceilings; zero is closed | `zero_capacity_closes_admission_without_becoming_unlimited` |
| Aliases, conflict, UTF-8 bytes, and one mount slot | `aliases_conflicts_text_bytes_and_mount_slots_stay_independent` |
| Pinned order, FIFO, and blocked-head bypass | `eligible_priority_fifo_bypasses_a_blocked_head` |
| Atomic reservation and overflow-safe arithmetic | `reservations_are_atomic_and_do_not_wrap` |
| Cancellation, deadline, supersede, and completion precedence | `stop_requests_keep_promises_until_actual_settlement` |
| Explicit offline reconnect and terminal outcomes | `offline_reconnect_is_explicit_and_other_failures_stay_terminal` |
| Retirement forgets aliases and does not restart numbers | `retirement_forgets_terminal_aliases_and_numbers_keep_advancing` |
| Dropped claim stays logically unsettled | `dropped_claim_remains_logically_unsettled` |
| Foreign and stale settlement does not release twice | `foreign_and_stale_settlement_does_not_release_twice` |
| Success and digest spelling are not authority | `success_and_digest_spelling_are_not_authority` |
| Joined submit, claim, and settle races | `joined_callers_admit_claim_and_settle_once` |
| Borrowed metadata is refused before queue-owned copying | `descriptor_capacity_refuses_before_queue_owned_copy` |
| Allocation window quiet probe and accepted copy | `allocation_window_admits_a_real_copy_and_stays_quiet` |

## Gaps That Are Not Queue Amendments

The 50 percent physical-memory ceiling, the waiting-for-memory process state, and the 2 GiB disk floor belong to later runtime integration.
This suite does not claim those controls and does not invent a waiting state on `Queue`.
`JobError::Poisoned` is not injected.
A sequence `Exhausted` result is not forced by `2^64` submissions.
The attempt-ceiling reconnect error variant is asserted only as an error with unchanged usage.
The queued-supersede terminal variant is asserted only as neither `Queued` nor `Unsettled`.
The allocation window counts this test process's `System` allocator while a thread-local flag is armed.
It is not resident-set size and not a whole-allocator policy.
A quiet arithmetic probe returned 0 and an 8192-byte vector probe counted at least 8192 before the refusal check was trusted.
No worker thread is started.
A dropped claim is a logical unsettled record, not an operating-system reaping event.
Independent key review has not accepted this suite.

## Checks

The package build used Rust 1.99.0, Cargo 1.99.0, `--locked --offline`, two Cargo jobs, a private target directory, a private `TMPDIR`, and the verified dictionary cache required by the toolchain.
`cargo fmt --all -- --check` passed.
`cargo test -p knowledge-core --all-targets --locked --offline -- --list` listed 58 tests and 0 benchmarks, with no ignored entry.
The 13 new tests are separate from the existing 45.
`cargo clippy -p knowledge-core --all-targets --locked --offline -- -D warnings` passed.
`cargo test -p knowledge-core --all-targets --locked --offline` passed the library harness (0 tests), `cache_contract` (16), `cache_owner` (3), and `checked_values` (7), then failed in `jobs_public_contract` with 11 passed and 2 failed.
Cargo stopped that command before `manifest_codec` and the doc harness.
A follow-up `--test manifest_codec` passed 19 tests, and `--doc` passed 0 doc tests.
Nothing was ignored.
The two jobs failures above are the committed result.
