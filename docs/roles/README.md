# Agent Roles

This directory defines harness-neutral role contracts. [Change execution](../change-execution.md) decides when to use them. Each role file owns its specific operation; this index owns the common contract and independence limits.

The primary is a position: it owns intent, integration, and progress for one scope. A role is a function. One agent may perform several functions where independence is not required.

## Common Contract

Read the root AGENTS.md, this common contract, the assigned role file, the named owners, and affected local guides. Derive expectations from source intent and accepted decisions. Inspect direct artifacts. Work only within the assigned paths and effects.

Return the conclusion, evidence pointers, exact checks and results, changed paths, unresolved findings, and decisions needed. Report missing capacity or authority before dependent work. Do not delegate again unless the assignment permits it.

## Functions

| Role | Question | Limits |
| --- | --- | --- |
| [Researcher](researcher.md) | What does independent evidence establish? | Research does not approve design. |
| [Consultant](consultant.md) | Is the need clear and the route justified? | Return governing choices to the engineer. |
| [Test author](test-author.md) | Which observation can disprove conformance? | Derive expectations independently of the candidate implementation. |
| [Builder](builder.md) | What coherent change meets the accepted contract? | Preserve contract-derived checks and unrelated work. |
| [Writer](writer.md) | Does the text preserve meaning and make the owner clear? | Another context checks a substantive rewrite. |
| [Reviewer](reviewer.md) | Is the actual result correct, complete, simple, and aligned? | Do not edit the work under review. |
| [Auditor](auditor.md) | Do source intent, decisions, owners, and observed behavior agree? | A finding or report cannot amend authority. |

## Independence

Self-review is useful but is not independent review. A new role name or fresh context does not erase authorship. A co-author cannot independently accept the same result.

A reviewer reports actionable findings by severity. Name the affected rule, consequence, evidence, and smallest sufficient correction. Distinguish introduced defects from existing findings.

If there are no actionable findings, say so and state the review's limits. Independent agreement remains a judgment, not proof that no defect exists.

A dispatch names the role file, source intent, accepted owners, evidence, exact writable paths, prohibited effects, completion checks, and return format. Each runtime reads the same contract; do not maintain harness-specific copies. Choose capability for the required judgment, not a fixed model identifier. All roles may raise a direction concern or request human involvement. Never prescribe a passing verdict or hide unresolved objections when steering or replacing a context.
