# Doctrine Review Corrections

Date: 2026-11-04. Decider: supervisor, by explicit acceptance of review findings F1–F6 and note N1, with specific F2, F5, and F6 rulings. This record supplements the [initial adoption decision](2026-11-04-engineering-doctrine-sync.md); operative rules remain in their owners.

## Decision

Correct all six findings and the provenance note from the independent review of commits `8b12276442afad1bdf0a0d32d20459824f7ece15` and `25f9d69c5e53763922a072a2ef94bdd0b8af381f`. The supervisor accepted those findings and authorized one documentation follow-up on fresh main. The source remains OpenKit `b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1`; no new source checkout or product decision is implied.

- F1: for material long-running work, require a fresh independent direction check after a reframe, long pause/resume, or primary replacement before renewed material investment. A selected meaningful evidence or stage trigger requires the check when reached. Ordinary commits within an examined route remain exempt.
- F2: every new mechanism needs a necessity statement and a reasoned ablation explaining what fails if it is dropped. Experimental removal is required only for uncertain necessity or removal claims, and for maintenance or deletion work. This corrects the initial universal experiment requirement; the underlying necessity question is already source doctrine.
- F3: separate admission and re-admission apply to distinct process, native-interface, artifact-smoke, and real-use acceptance harnesses. Ordinary module and in-process contract test files acquire no separate stand-in admission cycle. Their independent oracles and intended-failure evidence remain required.
- F4: duplicate-test retirement requires an existing passing check of the same owner clause at an equal or lower sufficient boundary, plus independent disposition.
- F5: two-pass OLD/NEW replacement proof is a local addition only for coupled tests of still-required behavior during behavior-preserving changes. Intended behavior changes use fail-OLD/pass-NEW instead. Pure internal-mechanism retirement uses independent disposition and coverage accounting, without invented public requirements or duplicate tests.
- F6: check the recorded reviewed-base-to-final-HEAD range with `git diff --check <reviewed-base>..HEAD`. Check staged and unstaged changes separately. An empty working diff does not inspect committed edits.
- N1: disjoint partitions, complete inventory union, actual coverage merge, original thresholds, and failure on missing, failed, or undeclared-skipped suites already exist in the source's release-gate context. Only the generic per-landing cadence and extension of accounting to local complete-suite batches are local additions.

Label stronger local worker additions explicitly in their operative owners: important-work independent authorship, scoped two-pass test replacement, structural inventories and ratchets, generic landing/batch and final-head policies, fix-forward quality assurance and severity labels, measurement controls, and the additional language restriction. Do not present them as equivalent upstream ports.

## Reason

The independent comparison found omitted source qualifiers and over-broad local requirements, not a product or toolchain blocker. Mandatory independent recovery must not become self-reconciliation. Removing test coupling must preserve required behavior without inventing tests for retired internals. A necessity explanation must not require an experiment for every already justified addition. Final-head evidence must inspect the committed candidate, and provenance must distinguish existing accounting invariants from their newly generalized application.

## Rejected Alternatives

- Retain optional recovery or trigger wording: rejected because it loses the independent observation the source requires.
- Require experimental removal for every mechanism: rejected by the explicit F2 ruling.
- Admit every ordinary test file through separate stand-ins: rejected because the source exempts lower-layer files from a distinct harness cycle.
- Require two green runs for intended changes or pure internal retirement: rejected by the explicit F5 scope ruling.
- Treat a clean working diff as committed whitespace proof: rejected because it can inspect no changed bytes.
- Call all local batch accounting missing upstream doctrine: rejected because the source already owns the accounting invariants in its release context.

## Revisit When

An independent comparison finds another lost qualifier, a scoped rule admits a concrete missed regression, or an explicit supervisor or engineer decision changes a local addition. This correction is not itself an independent re-review of the repaired artifacts.

## Affected Owners

- [Root execution contract](../../AGENTS.md), [engineering principles](../engineering-principles.md), and [change execution](../change-execution.md).
- [Verification](../verification.md), [test strategy](../specs/test-strategy.md), and [toolchain](../toolchain.md).
- [Test author](../roles/test-author.md), [builder](../roles/builder.md), [reviewer](../roles/reviewer.md), and [execution rationale](../change-execution-rationale.md).
- The [product delivery testing summary](../specs/product.md#delivery-and-qualification), as a pointer to the engineering lifecycle rather than another owner of retirement rules.
