# Local Git Transaction Framework

## Intent And Ownership

The engineer's 2026-11-05 M2 assignment requests real operation-bound local Git transactions, proposals, expected-ref comparison, and inspectable recovery in a dedicated clone at `40e8e01`.
[History](../specs/history.md) and [service](../specs/service.md) remain the behavior owners; [the crate guide](../../crates/knowledge-history/README.md) freezes this implementation seam.
Only the new history crate, this record, and the necessary workspace member/lock entry are owned here.
No shared service, storage, processing, kernel, authority, or root source is modified.

## Implementation Boundary

The backend creates a selected external app-owned bare repository, uses a private index and inert system-Git plumbing, persists deterministic operation intent, retains candidate trees/commits, and publishes target/operation refs with native compare-and-swap transactions.
An unchanged operation verifies the expected target and records a tree receipt without an empty operation commit.
Prepared and successful operation refs remain immutable; no reset, rollback overwrite, ref deletion, network transport, hooks, filters, or user configuration execution is exposed.
The caller's existing Core queue claim covers resource lifetime; this crate creates no independent admission queue or scheduler.

M1 must bind final file digests and visible journal results to this exact candidate before reporting `saved_local`.
Git-only `Published` does not prove atomic file/ref or multi-filesystem installation.
Native files-backend ref transactions lock and compare the whole set but do not prove crash-atomic multi-ref durability; one-sided interrupted facts remain indeterminate, not automatically replayed.

## Proof And Remaining Gates

Builder regressions use actual disposable repositories, native locked/competing refs, concurrent actual writers, and an exactly collected killed prepare process.
The first test run exposed an invalid bare-init environment; after its correction, retained RED proved stale-unchanged and one-sided ref interruption classification defects.
Both defects were corrected at production causes without changing their regression assertions.
Focused locked/offline tests, strict package Clippy, and workspace formatting are required for landing; raw artifacts and exact results live in `evidence/omni/m2-history-artifacts` outside the clone.
The complete headless workspace gate remains the integrating lead's obligation, not claimed by this scoped landing.

Required signing currently preserves candidate tree/intent and refuses with `SigningRequired`; signed production, remote synchronization, existing-repository adoption, service wiring, and durable-authority activation are not claimed here.
A milestone-end independent public-interface Git/durability review remains required before acceptance; the builder does not start or contact another agent.

## M2-S2-01 Fix Forward

The milestone review of `7956de4` found one material defect: an existing confirmed integration recipe bypassed proposal freshness on retry and its publication transaction did not verify the proposal ref.
The repair retains the exact confirmed proposal ref/OID, rejects obsolete unpublished retries, and includes native proposal verification in the accepted/operation ref transaction.
Candidate/recovery refs remain retained, and completed operation idempotency does not depend on later proposal freshness.
Real intervening expected-old proposal CAS plus reopen, valid new confirmation, completed retry after another proposal advance, and a competing proposal ref lock cover the repair; the two new regressions failed against `7956de4` and pass after the scoped correction.
The killed-process regression now selects its own real child mode instead of adding an ignored test entrypoint; its default body still performs prepare, exact child kill/wait, reopen, publish, and duplicate-operation assertions.
Prior source/11-case gate evidence remains unchanged; repair raw logs are separate under `evidence/omni/m2-history-artifacts/m2-s2-01`.
No local commit author attribution or generated/session trailer is added; product operation trailers are contract-required history facts, not contribution attribution.
