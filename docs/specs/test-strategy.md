# Test Strategy

This specification owns test boundaries, test authority and lifecycle, fixture hygiene, and regression selection. [Verification](../verification.md) owns oracle quality, harness admission, measurement evidence, and environment limits. Product contracts own expected behavior; this specification does not invent product requirements or change release authorization.

## Derive Tests From Owners

Tests project accepted owners and have no authority of their own. Derive expected values from the specification and public behavior, not the candidate implementation, constants, or helper output. Cite the owner clause for a production behavior change; a passing test is evidence, never the reason for that change. Return a check-contract conflict to its owner instead of bending production to an unsupported expectation.

As a stronger local addition, for important implementation work, assign a test author independent of the builder. The author reads the specification and public surface rather than using the candidate as the oracle. For a representation crossing, the author reads the owner on each side. Fix expectations before the candidate exists when the intended behavior is known. If the failure, environment, or oracle is unknown, probe first and then write the lowest sufficient lasting regression.

Normally give each invariant one primary regression at the lowest sufficient boundary. Add at most one higher-layer check for a distinct integration risk, not another copy of the same assertion. Inspect existing coverage before adding a check; extend or replace an existing primary regression when it proves the same invariant.

## Units Are Public Module Interfaces

A unit is a module's public interface, not each function or class. A behavior-preserving internal refactor should leave its unit tests passing. A failure caused only by a changed internal mechanism is a test defect, not evidence for restoring that mechanism.

- Pure parsers, algorithms, and converters need fine-grained tests because their interface is their behavior.
- Stateful owners need tests through caller operations with the real in-process store and disposable filesystem state.
- Glue and adapters need contract or boundary tests rather than a test for every forwarding function.

Doubles replace effects the subject does not own, such as an external model, network, clock, or process. Do not double a stateful subject's own collaborators to claim storage, recovery, or concurrency proof. Use an in-memory substitute only when persistence, recovery, and concurrency are not the behavior under test. Observe internals only when no public result reveals a required responsibility, and name that responsibility.

Assert observable results, owner-read persisted facts, public status, required events, and refusals. Pin exact bytes, counts, ordering, or wording only when an owner requires them; name the clause beside the assertion. Do not freeze helper names, private layout, incidental call order, unordered result order, or diagnostic wording without such authority.

## Prove Boundaries With Real Owners

For a representation crossing, compose the real producer and real consumer in process without a double on either side. A double may stand further out for an effect neither side owns. Shared schemas and fixtures prove shape; they do not alone prove agreement in behavior.

Exercise encoding, identity, ordering, chunk splitting, or durable handoff where the changed contract requires them. Inject relevant failures between steps, restart with reopened state, duplicate delivery, and reordering. Assert the crossing's invariant rather than accumulating isolated examples or one disconnected test per side. Do not import private implementation into a black-box process test to bypass the public boundary it claims to prove.

## Test Authority And Lifecycle

Before migrating any failing test, classify its failure:

| Class | Meaning | Action |
| --- | --- | --- |
| Setup-only | The route to the same valid scenario moved. | The builder may migrate setup mechanically and run focused checks. |
| Test defect | The test pins a replaced internal mechanism or contradicts an accepted owner. | An independent test author decides its disposition. |
| Product defect | The product violates accepted behavior. | Fix the product with its own lowest sufficient regression. |

Setup-only migration preserves the scenario, relevant preconditions, injected faults, real subject, public boundary, and every assertion's semantics. Helper assertions, expected values, bounds, and test-name meaning also stay unchanged. Show that the deciding assertions still run. This migration needs neither a per-case oracle request nor a separate test-author or review round. A change to expectations is not setup-only, even when a helper hides it.

An independent test author may rewrite or retire a test when it pins a removed mechanism, contradicts an accepted owner, or duplicates an existing passing check at an equal or lower sufficient boundary that proves the same owner clause. Record each disposition with its owner clause and covering check, or the retired mechanism and accepted reason. The builder whose change fails a test cannot decide its retirement or change its existing assertions, expected values, or bounds. Never weaken, delete, skip, or bypass a contract-derived failure to obtain green. Never regenerate a replacement oracle from the candidate implementation.

Local addition: when replacing a coupled test for still-required behavior during a behavior-preserving change, delete the old test only after an independently authored public-interface replacement passes on both OLD and NEW. Retain both code identities, exact commands, deciding assertions, results, and the disposition. This two-pass proof preserves required behavior without preserving internal coupling; it does not govern the distinct cases below.

For an intended behavior change, use a specification-first regression that fails on OLD and passes on NEW for the intended reason, instead of requiring two passes. An accepted removal of behavior records the ended owner requirement and the independent disposition; do not claim unchanged-behavior proof for intentionally incompatible semantics.

For pure internal-mechanism retirement, the independent author's disposition and coverage accounting are sufficient. Record the retired mechanism, accepted reason, and which passing checks cover any still-required owner clauses; state explicitly when no public requirement remains. Do not invent a public requirement or add a duplicate test merely to obtain two green runs. Existing untouched tests need no speculative whole-suite rewrite.

Before refactoring uncovered behavior, characterize its current public behavior. Report a conflict with accepted intent separately; do not silently fix it or freeze it as the intended contract.

## Reach State Through Shared Builders

Outside an owner's own tests, reach state through owner operations or shared test-support builders over those operations. Do not write another owner's raw storage, use private setters, or fabricate durable records and receipts. When an owner changes setup, change the shared builder once instead of hand-writing setup in every file. A valid state no owner can produce is a missing operation or an impossible state; report it. Deliberate corruption or interruption after valid setup remains allowed when it proves a required refusal or recovery.

Use disposable roots, fake secret markers, dynamic ports, deterministic inputs, and state-based waits. Do not depend on user libraries, persistent application data, existing profiles, or real credentials in the ordinary suite. Close files and stores, remove owned temporary directories, and settle child processes on success and failure. A killed process may require separately authorized temporary-storage reclamation. Real models, subscriptions, external networks, and quota-consuming tests remain explicit opt-ins. Once selected, a test fails when a prerequisite is missing; a runtime error is not permission to skip.

## Choose A Proof Boundary

| Boundary | What it proves | Omni application |
| --- | --- | --- |
| Static and repository | Formatting, lint, structure, links, and contract consistency. | Cargo formatting and lint; documentation checks. |
| Module | Public behavior of one owner. | Rust tests through public operations and real local stores. |
| Contract | Agreement across real producers and consumers. | Library, CLI, MCP, storage, and retrieval crossings where affected. |
| Process | Boot, transport, persistence, and restart. | Built CLI or stdio MCP with disposable roots. |
| Native interface | Visible user workflows and native integration. | Actual supported macOS application; source inspection is insufficient. |
| Artifact smoke | Minimal startup and useful health. | Exact built CLI or app artifact, not a development-only substitute. |
| Real use | A realistic user outcome beyond deterministic coverage. | Authorized native or model workflow, reported separately. |

These are proof boundaries, not a mandatory checklist or a new runner architecture. Use existing Rust test and build infrastructure. Reduce a confirmed real-use defect to the lowest sufficient deterministic regression, or record why reduction is unavailable. A shallow smoke test must not grow into the full workflow suite.

## Match Depth To Consequence

Authorization, credentials, containment, data loss, durable authority, and irreversible effects need strict negative and adversarial coverage. Persistence, migrations, idempotency, and duplicate-effect prevention need focused transition and restart evidence. Ordinary availability, cleanup, and diagnostics need representative success and the documented safe fallback, not every state permutation. Deferred scale, compatibility, or transparent recovery needs no implementation or tests until an accepted current contract requires it. Do not add dependencies or broader effect access merely to make a test convenient.

## Keep Coverage And Cost Honest

A suite's value is fault detection, not test count or line coverage. Coverage thresholds do not justify meaningless tests around generated output or forwarding glue. Report changed test and production lines; explain a test-to-production ratio above three to one. This ratio is a review signal, not a target or gate. For documentation-only work, report both code counts as zero rather than inventing a ratio.

Source-derived accounting invariants: partitions must be disjoint and their union must cover the intended inventory. Missing, uncollected, failed, or undeclared-skipped suites fail the gate. When coverage is partitioned, merge actual observations and apply the original whole-suite thresholds without weakening them. These invariants already exist in OpenKit's release-gate context; they are not missing upstream doctrine.

Local addition: apply those invariants to local complete-suite batches, accounting for every selected case, its exact code identity, result, and declared opt-in exclusions. Do not call partial or mixed-commit evidence a complete passing gate. The generic changed-plus-related-test cadence at each landing and this extension to local batches are local additions. [Change execution](../change-execution.md) owns per-landing selection, final-head gates, and fix-forward handling.
