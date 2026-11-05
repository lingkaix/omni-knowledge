# Preparatory Cooperative Jobs Contract Tests

This record owns independent public-contract tests for the portable cooperative job queue at frozen head
`09b73a18e231798859e457cef41ade4d08197d1e` on branch `omni-jobs-independent-65e49be`.
It does not accept the queue, amend storage, activate a caller or executor, or claim durable restart idempotency.
The tests are `crates/knowledge-core/tests/jobs_contract.rs`.
Production lines in this change: 0. Test lines: 906. No manifest, lock, owner, trait, or implementation edit.

## Authority And Independence

Expected observations come from the accepted [storage owner](../specs/storage.md#bounded-jobs-and-cache),
the accepted [effective resource accounting policy](../specs/storage.md#effective-resource-accounting),
the [bounded jobs change record](2026-11-04-bounded-jobs.md), the
[preparatory cooperative jobs guide](../../crates/knowledge-core/README.md#preparatory-cooperative-jobs),
and the rendered public Rustdoc for `knowledge_core::jobs` plus `backend::Budget` and `data::ContentDigest`.
The candidate `src/jobs.rs` body, builder reports, smoke results, probes, and oracle tables were not inputs.
The subject is the real in-process `Queue`, `Claim`, and reservation account. There is no fake executor: a claim is
settled only through public settlement, and abandonment remains explicitly unsettled.
A failing check stays a failing check. This record does not retune an expectation to obtain a pass.

## What The Suite Decides

`Queue::new` refuses `max_pending` above `max_records` and accepts equal bounds. Every zero capacity closes the
corresponding admission or claim instead of becoming unlimited, including zero per-mount slots.
Identical retained operation delivery is a no-op, a new operation/job alias shares one canonical job and consumes a
receipt, and changed budget/reservation/priority/attempt/key intent under a retained operation returns `Conflict`.
The pending bound is separate from retained-record capacity and frees on claim. Receipt capacity bounds every alias;
explicit terminal retirement forgets the whole job and all aliases, frees descriptor text, and does not reuse the
retired instance-local job number.
Descriptor text is charged before copying, so an oversized borrowed identity refuses without retaining queue text.
Per-mount slots are scoped by the composite `(context, library, mount)` and eligible work bypasses work blocked on a
held slot. Claims order pinned before normal and keep original admission order among eligible ties.
Cancellation and elapsed-deadline observation record the first cooperative request without releasing reservations;
the first reason remains recorded until trusted settlement. A dropped claim performs no settlement and deliberately
strands its promise. Wrong-instance, wrong-context, and stale claims release nothing twice.
Offline settlement should expose the public `WaitingOffline` state, after which reconnect is explicit and bounded by
finite attempts. Invalid-model settlement is terminal for reconnect.
Missing jobs report `Unknown`, mismatched facts report `Context`, active jobs refuse retirement with `State`, and
retirement ends the volatile idempotency window. Empty identities and a zero attempt bound report `Invalid`.
Supersession requires the same subject and a different full key, never digest ordering.

| Owner or public clause | Test |
| --- | --- |
| `new` pending/record consistency; zero closes admission | `new_rejects_pending_above_record_capacity_and_accepts_equal_bounds`, `zero_capacities_close_admission_and_never_mean_unlimited` |
| Identical delivery no-op, aliases, changed-intent `Conflict` | `retained_identity_reuse_is_idempotent_or_conflicting` |
| Pending bound separate and released on claim | `pending_capacity_is_separate_and_frees_on_claim` |
| Alias receipts bounded; retirement forgets the group | `alias_capacity_is_bounded_and_retirement_forgets_every_alias` |
| Descriptor text charged before copying | `descriptor_capacity_refuses_borrowed_metadata_before_copying` |
| Composite per-mount slot and blocked-work bypass | `per_mount_slots_are_composite_and_eligible_jobs_bypass_blocked_work` |
| Pinned-before-normal and FIFO ties | `claim_orders_pinned_before_normal_and_fifo_within_priority` |
| Cancellation/deadline requests hold reservations | `cancellation_and_deadline_requests_hold_reservations_until_settlement` |
| Offline wait and finite reconnect attempts | `offline_settlement_waits_for_explicit_bounded_reconnect` |
| Invalid model is terminal for reconnect | `invalid_model_settlement_is_terminal_for_reconnect` |
| Wrong instance/context/stale claim release nothing | `wrong_instance_stale_and_context_claims_release_nothing_twice` |
| Claim drop strands capacity | `dropped_claim_strands_its_reservation_until_another_settlement` |
| `status`/`cancel`/`retire` distinguish `Unknown`, `Context`, `State` | `status_and_retirement_distinguish_unknown_context_and_state` |
| Empty identity or zero attempts is `Invalid` | `invalid_identities_and_zero_attempts_are_refused` |
| Same-subject, different-key supersession | `supersession_requires_same_subject_and_different_key` |
| Claim and status expose admitted budget/reservation/deadline | `admitted_claim_reports_the_unchanged_budget_reservation_and_deadline` |

## Reported RED

The final run selected 17 tests, passed 16, failed 1, and ignored 0.
`offline_settlement_waits_for_explicit_bounded_reconnect` is the RED.

At this frozen head, settling a first attempt with `Outcome::Offline` returned `State::Finished(Outcome::Offline)`.
The public Rustdoc for `State::WaitingOffline` says "Actual offline settlement released reservations; reconnect is
explicit and attempt-bounded", and `Outcome::Offline` says "only explicit reconnect may queue another bounded attempt".
The test supplied `max_attempts: 2` and observed the first attempt, so a bounded reconnect was still available.
It asserts `State::WaitingOffline` and fails with:

```text
thread 'offline_settlement_waits_for_explicit_bounded_reconnect' panicked at crates/knowledge-core/tests/jobs_contract.rs:637:5:
assertion `left == right` failed
  left: Finished(Offline)
 right: WaitingOffline
```

No production repair and no expectation retune were performed. Because the test stops at this first assertion, its
later reconnect and attempt-bound assertions did not execute. Offline/reconnect behavior therefore remains unverified
at this head.

## Test-Author Setup Failures Kept In The Evidence

The first compile attempt failed with three test-code errors, not producer failures: `Snapshot` has no `PartialEq`
implementation, so two `assert_eq!` calls on `Result<Snapshot, JobError>` were invalid, and `Facts::with_digests` moved
its receiver before a later borrow. The test code was corrected to use `matches!` for those error results and to clone
the facts before replacing digests. The first full execution then compiled and ran all 17 tests.
`cargo fmt --all -- --check` initially reported formatting diffs in this test file; `cargo fmt --all` was applied and the
check passed. These setup corrections are recorded separately from the producer RED.

## Checks

The package build used Rust 1.99.0 and Cargo 1.99.0, `--locked --offline`, two Cargo jobs, the private target
`/tmp/opencode/omni-jobs-tests/target`, private `TMPDIR`, and the pinned IPADIC cache supplied by
`tools/kernel-probe/prepare-dictionary.py`.

| Command | Exit |
| --- | --- |
| `cargo fmt --all -- --check` | 0 |
| `cargo clippy -p knowledge-core --test jobs_contract --locked --offline -- -D warnings` | 0 |
| `cargo test -p knowledge-core --test jobs_contract --locked --offline` | 101 (16 passed, 1 failed, 0 ignored) |

`git diff --check` over tracked changes exited 0, and both untracked assigned files reported no whitespace-error lines
with `git diff --no-index --check /dev/null <path>`. The tests and this record remain uncommitted in the
assigned worktree; nothing was pushed. No workspace-wide test, GUI run, network access, commit, or production edit was
performed.

## Gaps That Are Not New Rules

The suite does not pin an exact terminal state for queued supersession. The public texts allow the first stop request
to appear as an immediate terminal `Superseded` result or as an `Unsettled` first cancellation reason, and the test
accepts either without promoting the observed branch into a rule.
The suite also does not pin the exact `JobError` variant for reconnect past the attempt bound; it requires a refusal and
unchanged accounting, because the public texts distinguish finite attempts from the `Exhausted` overflow condition.
No uninterrupted offline/reconnect success is demonstrated while the reported RED stands.
This suite does not exercise poisoned state, monotonic-clock overflow, concurrent threads, durable restart
reconciliation, physical RAM/disk enforcement, cache retirement, executor settlement, or any caller activation.
A passing portable queue counter is not evidence that an opaque processor was physically bounded.
