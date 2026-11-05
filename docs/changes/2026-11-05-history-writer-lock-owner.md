# Original-Process Git Writer Lock Lifetime

## Finding And Scope

Hosted C2 run `41978090775`, Linux job `127081567205`, reported 11 passes and two real-Git `Busy` failures in 1.54 seconds: `hostile_inputs_config_and_native_binding_refuse` at the sequential publish helper and `isolated_proposal_and_exact_owner_confirmation` at confirmation. These were not deadline failures or a reopening of the closed stale-proposal CAS finding.
The source evidence is `evidence/omni/lead-artifacts/trunk-integration-0105/hosted-c2-linux-evidence/tests.log` (SHA-256 `0be5cda843344069a4c3b85f5448ea3dff8cca26ae82022c85941e07aba6928d`).
Closing a flock file only releases the lock after every duplicate of its open-file description has closed. An unrelated concurrently spawned child can therefore prolong a completed logical writer. Conversely, an inherited guard must not explicitly unlock its still-live parent's shared description.

## Repair

The existing [History](../../crates/knowledge-history/README.md) seam now wraps the acquired descriptor in `OwnedWriterLock`, records its acquiring PID, and explicitly unlocks on original-process Drop only. This is an exclusive lock lifetime fix, not a retry, added wait, raised budget, scheduler, ref protocol, or second Git engine.
An unresolved native worker must instead retain the actual descriptor until its own collection; lexical completion is not permission to release unknown worker ownership. The descriptor-transfer control deliberately keeps the kernel lock Busy until explicit known release. The completion lane's native-worker integration remains a separately qualified implementation obligation.

## Consequential Review Input

The deterministic regression execs this test's finite native helper using `Command::pre_exec` and real `dup2` of the actual flock descriptor. There is no raw multithreaded Rust fork, fake Git, ignored helper test, or timing-only race dependency.
It proves historical close-only Busy, active-writer exclusion, next-writer acquisition while an unrelated helper still holds the inherited descriptor, child-process guard Drop not unlocking the live parent, and actual retained-descriptor exclusion.
Two private source counterfactuals fail the unchanged control: close-only Drop reproduces `WouldBlock`; unconditional cross-PID unlock incorrectly admits a competing writer. Each uses a distinct private Cargo target to prevent copied-tree artifact reuse. Initial shared-target setup logs are retained but are not evidence for the second counterfactual.
All protected 13 real-Git cases and their assertions/budgets are unchanged; the read-side two cases also remain. Native proposal verification in the publication ref transaction, completed-operation idempotency, and expected-ref conflict behavior are unchanged.
Review this lock boundary with the M2 completion milestone alongside native child collection, held-file bundle admission, signing, adoption, and transport. Passing this regression is not a claim that those other mechanisms are finished or independently reviewed.
