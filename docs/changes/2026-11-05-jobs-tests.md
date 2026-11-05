# Preparatory Cooperative Jobs Contract Tests

This record owns independent public-contract tests for the portable cooperative job queue at frozen head
`09b73a18e231798859e457cef41ade4d08197d1e` on branch `omni-jobs-independent-65e49be`.
It does not accept the queue, amend storage, activate a caller or executor, or claim durable restart idempotency.
The tests are `crates/knowledge-core/tests/jobs_contract.rs`.
Production lines in this change: 0. Test lines: 942. No manifest, lock, owner, trait, or implementation edit.

## Authority And Independence

Expected observations come from the accepted [storage owner](../specs/storage.md#bounded-jobs-and-cache),
the accepted [effective resource accounting policy](../specs/storage.md#effective-resource-accounting),
the [bounded jobs change record](2026-11-04-bounded-jobs.md), the
[preparatory cooperative jobs guide](../../crates/knowledge-core/README.md#preparatory-cooperative-jobs),
and the rendered public Rustdoc for `knowledge_core::jobs` plus `backend::Budget` and `data::ContentDigest`.
The author also read the producer change record `2026-11-04-bounded-jobs.md` even though the assignment excluded
producer change records. It contains producer rationale and implementation descriptions, not private probe outcomes or
magic expected fixture values. The candidate `src/jobs.rs` body, builder reports, smoke results, probes, and oracle
tables were not read. Because the change record was read, this pool is disclosed external regression coverage, not
clean independent qualification. A fresh author and clone must derive a separate public-contract pool without this
record, the first pool, or the lead finding.
The subject is the real in-process `Queue`, `Claim`, and reservation account. There is no fake executor: a claim is
settled only through public settlement, and abandonment remains explicitly unsettled.
The initial RED was later classified as a test-side final-attempt oracle error; the correction is recorded below.
No expectation is retuned merely to obtain a pass, and no production line is changed.

## What The Suite Decides

`Queue::new` refuses `max_pending` above `max_records` and accepts equal bounds. Every zero capacity closes the
corresponding admission or claim instead of becoming unlimited, including zero per-mount slots.
Identical retained operation delivery is a no-op, a new operation/job alias shares one canonical job and consumes a
receipt, and changed budget/reservation/priority/attempt/key intent under a retained operation returns `Conflict`.
The pending bound is separate from retained-record capacity and frees on claim. Receipt capacity bounds every alias;
explicit terminal retirement forgets the whole job and all aliases, frees descriptor text, and does not reuse the
retired instance-local job number.
Oversized borrowed descriptor metadata is refused without additional retained queue-owned accounting; this retained
counter does not establish actual allocation order or live allocation.
Per-mount slots are scoped by the composite `(context, library, mount)` and eligible work bypasses work blocked on a
held slot. Claims order pinned before normal and keep original admission order among eligible ties.
Cancellation and elapsed-deadline observation record the first cooperative request without releasing reservations;
the first reason remains recorded until trusted settlement. A dropped claim performs no settlement and deliberately
strands its promise. Wrong-instance, wrong-context, and stale claims release nothing twice.
A non-final offline settlement exposes the public `WaitingOffline` state and releases its charges, after which
reconnect is explicit and bounded by finite attempts. The final permitted attempt settles `Finished(Offline)`,
releases the reconnected claim's charges, reports two attempts, and refuses further reconnect without changing
observed usage or snapshot fields. Invalid-model settlement is terminal for reconnect.
Missing jobs report `Unknown`, mismatched facts report `Context`, active jobs refuse retirement with `State`, and
retirement ends the volatile idempotency window. Empty identities and a zero attempt bound report `Invalid`.
Supersession requires the same subject and a different full key, never digest ordering.

| Owner or public clause | Test |
| --- | --- |
| `new` pending/record consistency; zero closes admission | `new_rejects_pending_above_record_capacity_and_accepts_equal_bounds`, `zero_capacities_close_admission_and_never_mean_unlimited` |
| Identical delivery no-op, aliases, changed-intent `Conflict` | `retained_identity_reuse_is_idempotent_or_conflicting` |
| Pending bound separate and released on claim | `pending_capacity_is_separate_and_frees_on_claim` |
| Alias receipts bounded; retirement forgets the group | `alias_capacity_is_bounded_and_retirement_forgets_every_alias` |
| Oversized descriptor text refused without added retained accounting | `descriptor_capacity_refuses_oversized_metadata_without_retaining_text` |
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

## Classified RED And Test-Only Correction

The first frozen run (commit `53308cd6`) selected 17 tests, passed 16, failed 1, and ignored 0. The original report
misidentified the failed assertion as the first offline settlement. It was the **second/final** settlement under
`max_attempts: 2` at `jobs_contract.rs:637`. The first settlement already returned `WaitingOffline`; its waiting-count
and reservation-release assertions passed. Explicit reconnect succeeded and the second claim succeeded before the
failed assertion. The original final-attempt oracle incorrectly required another `WaitingOffline`; actual behavior was
`Finished(Offline)`. The original exact failure is retained:

```text
thread 'offline_settlement_waits_for_explicit_bounded_reconnect' panicked at crates/knowledge-core/tests/jobs_contract.rs:637:5:
assertion `left == right` failed
  left: Finished(Offline)
 right: WaitingOffline
```

The lead classified this as an overstrict test-side terminal-state/waiting-count oracle, not a producer bug. The
accepted owner and public guide permit waiting/reconnect only within finite caller-selected attempts and do not
require a retryable waiting state after attempts are exhausted.

A separate test-only correction follows the frozen test commit. It retains the first waiting/reconnect positive path
and now reaches these final-attempt assertions: the second claim re-holds the `200/100` reservation; final settlement
returns `Finished(Offline)`; `usage.pending`, `usage.unsettled`, and `usage.waiting_offline` are zero; `usage.reserved`
is the zero reservation; `snapshot.state` is `Finished(Offline)` and `snapshot.attempts` is 2. A further reconnect is
refused. The complete `Usage` before and after refusal compare equal, and the snapshot's state, attempts, reservation,
and deadline compare equal after refusal. These fields are compared before/after rather than assigned new terminal
values. The corrected run passes all 17 tests.

An intermediate correction run pinned `snapshot.reservation` to zero after final settlement and failed because that
field still reported the admitted promise. That assertion was not a required public release observation; the released
account is `usage.reserved`. The intermediate log is retained, and the final assertion compares the snapshot
reservation before and after the refused reconnect instead of promoting its terminal value.

## Test-Author Setup Failures Kept In The Evidence

The first compile attempt failed with three test-code errors, not producer failures: `Snapshot` has no `PartialEq`
implementation, so two `assert_eq!` calls on `Result<Snapshot, JobError>` were invalid, and `Facts::with_digests` moved
its receiver before a later borrow. The test code was corrected to use `matches!` for those error results and to clone
the facts before replacing digests. The first full execution then compiled and ran all 17 tests.
`cargo fmt --all -- --check` initially reported formatting diffs in this test file; `cargo fmt --all` was applied and the
check passed. These setup corrections are recorded separately from the later classified final-attempt test oracle.

## Checks

The package build used Rust 1.99.0 and Cargo 1.99.0, `--locked --offline`, two Cargo jobs, the private target
`/tmp/opencode/omni-jobs-tests/target`, private `TMPDIR`, and the pinned IPADIC cache supplied by
`tools/kernel-probe/prepare-dictionary.py`.

| Command | Exit |
| --- | --- |
| `cargo fmt --all -- --check` | 0 |
| `cargo clippy -p knowledge-core --all-targets --locked --offline -- -D warnings` | 0 |
| `cargo test -p knowledge-core --test jobs_contract --locked --offline -- --list` | 0 (17 tests) |
| `cargo test -p knowledge-core --test jobs_contract --locked --offline` | 0 (17 passed, 0 failed, 0 ignored) |

The original frozen run at commit `53308cd6` exited 101 with 16 passed, 1 failed, 0 ignored. A separate test-only
correction commit directly follows it with the same frozen production source. The worktree was clean after that commit
and `git diff --check` between the frozen source and the correction head exited 0. Nothing was pushed. No
workspace-wide test, GUI run, network access, or production edit was performed.

## Gaps That Are Not New Rules

The suite does not pin an exact terminal state for queued supersession. The public texts allow the first stop request
to appear as an immediate terminal `Superseded` result or as an `Unsettled` first cancellation reason, and the test
accepts either without promoting the observed branch into a rule.
The suite also does not pin the exact `JobError` variant for reconnect past the attempt bound; it requires a refusal and
unchanged accounting, because the public texts distinguish finite attempts from the `Exhausted` overflow condition.
The offline test demonstrates waiting, explicit reconnect, a second claim, final terminal settlement, and accounting
release sequentially. It does not prove concurrent atomic reservations.
The descriptor case observes that sequential `usage.descriptor_bytes` is unchanged after an oversized refusal. That
retained-counter observation is not proof that the live allocation happens only after capacity acceptance, nor proof
of any allocator or live-allocation ordering.
This suite does not exercise actual joined threads or barriers, poisoned state, monotonic-clock overflow, durable
restart reconciliation, physical RAM/disk enforcement, cache retirement, executor settlement, or any caller
activation.
This first pool is disclosed external regression coverage, not a clean independent qualification. A fresh author and
clone must author the independent pool.
A passing portable queue counter is not evidence that an opaque processor was physically bounded.
