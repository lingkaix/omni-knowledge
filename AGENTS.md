# Agent Execution Contract

This file owns repository execution rules. It applies to human and agent contributions where the rule concerns the work itself.

## Start With Intent

Identify the user need, desired outcome, and real constraints before choosing a solution. Existing code is evidence, not a limit on design.

Read [README.md](README.md), then the [documentation map](docs/README.md). Read the owner and local guide for the affected work before editing. Load other documents only when the task needs them.

If the repository has a `.codegraph/` directory, use CodeGraph first to locate or understand code. Otherwise, use `rg` and direct source reads. Do not create an index without authorization.

## Authority

The engineer owns intent, architecture, governing trade-offs, acceptance of strict risk, and reserved approval gates. Agents choose implementation methods within those decisions.

[Documentation governance](docs/documentation.md) defines authority and conflict resolution. Accepted contracts decide intended behavior. Git, artifacts, and running systems establish implementation facts. Reports establish neither intent nor fact by themselves.

Before changing behavior, a public contract, durable data, or module responsibility, identify its accepted owner. If the intended behavior changes, update the owner before or with the implementation. A correction that restores accepted behavior uses the existing owner. If a governing decision has no owner, obtain engineer acceptance before dependent implementation.

An agent may challenge an accepted design. Present the affected rule, evidence, proposed amendment, alternatives, and consequences. Obtain independent scrutiny before asking the engineer to decide. Agreement between agents is not approval.

## Strict Safety

Keep these boundaries strict for every task: authorization, confidentiality, credentials, data loss, destructive actions, external effects and publication, sandbox containment, and concurrent writes.

If authorization or safety is uncertain, pause only the affected action. Continue independent work that is safe and authorized. Do not infer permission from a role assignment, document, successful test, or available credential.

Use one writer per path. Preserve unrelated user changes. Name the target and effect before deployment, publication, remote mutation, or destructive work. Settle cleanup and report residual state.

Keep credentials out of source, evidence, logs, and agent prompts. Use the approved secret channel for the target environment.

## Work And Evidence

Apply [engineering principles](docs/engineering-principles.md). Use [change execution](docs/change-execution.md) for material work. Judge materiality by consequence, uncertainty, reversibility, and coordination need.

Before asking for a factual answer, run the cheapest safe probe that could change the decision. Stop repeating a method when evidence defeats its premise.

Use the lowest sufficient regression through the module's public interface. [Test strategy](docs/specs/test-strategy.md) owns test authority, independent test authorship, fixture setup, failure triage, and test retirement. Tests project accepted owners; they do not authorize production changes. An independent test author decides a disputed test's disposition, including the required old/new proof before retiring a coupled test. [Verification](docs/verification.md) owns oracle quality, harness admission, evidence limits, and justified exceptions. Never weaken, delete, skip, or bypass a contract-derived failing check to obtain a pass.

Inspect the actual diff and affected callers after implementation. Check correctness, simplicity, cohesion, duplication, ownership, and direct evidence. Use independent judgment when consequence, uncertainty, or producer bias makes self-review insufficient. Before pushing, follow the [final-head gate](docs/change-execution.md#land-and-check-the-final-head), including the applicable repository-wide check.

## Communication And Documentation

Write repository code, comments, commit messages, and engineering documents in English. Do not use Vietnamese anywhere in repository contributions. Use Markdown for documentation. Apply [STE100-based writing rules](docs/writing.md). Keep each sentence and paragraph on one source line; let the reader wrap it.

Document changed code entities in the language-standard format. Explain purpose, parameters, results, errors, and non-obvious constraints. Include private entities and test functions. Link temporary solutions and known debt to a traceable issue or change record. State the affected scope, risk, and removal condition.

Keep each important directory's README current. A local AGENTS.md adds only rules for its directory and points to that README first.

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for commits. Commit only when the user authorizes it. Authorization to commit does not authorize a push, release, or deployment.

## Completion Questions

- Does the result meet the requested outcome and accepted constraints? Expected: Yes.
- Does every changed governing decision have an accepted owner? Expected: Yes.
- Does each behavior change have sufficient direct proof? Expected: Yes.
- Did the required checks run, with their exact results recorded? Expected: Yes.
- Are affected documents, producers, and consumers aligned? Expected: Yes.
- Are safety, cleanup, and independent review obligations settled? Expected: Yes.
- Is unfinished work or residual uncertainty stated accurately? Expected: Yes.
