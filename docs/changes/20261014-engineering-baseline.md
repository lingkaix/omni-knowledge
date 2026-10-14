# Engineering Baseline Adoption

Status: complete. Date: 2026-10-14. This record preserves intent and evidence; current rules live in the linked owners.

## Source Intent

The engineer requested extraction of general engineering principles, standards, processes, and methods from OpenKit into Proxy Gate and Omni Grep. The engineer requested STE100-style language.

The engineer then required review of all existing Proxy Gate engineering rules against the new principles. The engineer authorized initialization and the first Omni Grep commit. The engineer deliberately left Omni Grep undefined to test whether the extracted rules are general.

These statements are English summaries of the engineer's Chinese instructions in the initiating conversation. They authorize engineering guidance and local repository preparation. They do not select product behavior or authorize deployment.

## Source And Selection

Source: OpenKit at commit `dab3a0920254a536ca7dd5c1fd50e303ab4dbcd2`. The source working tree was clean when inspected. Source paths below identify provenance, not a runtime dependency or authority over this repository.

| Source in OpenKit | Criterion retained | Local owner |
| --- | --- | --- |
| AGENTS.md FIRST-001 and Build Loop | Derive the solution from need and constraints; revise methods from evidence. | [Principles](../engineering-principles.md), [execution](../change-execution.md) |
| AGENTS.md AUTHORITY, AUTH-001, and PRECEDENCE clauses | Human governing authority, accepted owners, explicit conflict resolution. | [Root contract](../../AGENTS.md), [documentation](../documentation.md) |
| AGENTS.md Safety Kernel and SCOPE-004 | Eight strict boundaries, local pauses, one writer per path. | [Root contract](../../AGENTS.md) |
| AGENTS.md QUALITY clauses and engineering-doctrine.md | SOLID, KISS, DRY, YAGNI, deep interfaces, discoverability, present need, and measured simplification. | [Principles](../engineering-principles.md) |
| AGENTS.md DOC-017 and writing.md | Definition, data authority, lifecycle, failure semantics, and observable acceptance. | [Documentation](../documentation.md) |
| change-execution.md | Source intent, revisable checkpoint, useful next action, reframe, human decision, retention, and evidence-based closeout. | [Execution](../change-execution.md) |
| roles/ and agent-harnesses.md | Bounded dispatch, capability, independent review, direct artifacts, and runtime restrictions. | [Roles](../agent-roles.md), [execution](../change-execution.md) |
| verification-instruments.md and AGENTS.md TEST clauses | Test-first default, sufficient regression, sound oracle, deliberate failure, effect-domain limits, and exact results. | [Verification](../verification.md) |
| documentation-model.md and writing.md | One owner per rule, intent versus fact, acceptance versus implementation, lossless rewriting, and local navigation. | [Documentation](../documentation.md) |
| writing.md and COMM-001 | Plain active language, stable terms, sentence targets, retained qualifiers, and no prose line wrapping. | [Writing](../writing.md) |
| engineering-doctrine.md and core/contract-evolution.md | Deliberate continuity and extension semantics; no accidental contract assumption. | [Principles](../engineering-principles.md) |

## Deliberate Exclusions

OpenKit's data-only compatibility policy, SQLite history requirement, NanoCore and NanoHost concepts, and product state machines are not general engineering rules. Each product must own its own compatibility contract.

The baseline does not import OpenKit's L0-L6 test taxonomy, container declarations, host manifests, seven-section schemas, word ceilings, finding grammar, or pilot-specific audit machinery. It preserves their general purpose through proportionate evidence, explicit effects, and independent judgment.

The baseline does not bind model versions, agent providers, a fixed role sequence, or coordination software. Runtime selection remains an execution choice under applicable workspace instructions. No document generator, test runner, registry, or enforcement script is added without current need.

## Acceptance Observations

- Both repositories have self-contained engineering guidance with resolvable local links.
- Omni Grep defines no product, implementation language, framework, or deployment model.
- Proxy Gate's existing engineering rules are reconciled; retained product constraints are identified separately.
- Changed prose uses the stated STE100-based style, with no claim of full dictionary compliance.
- Independent review compares the actual documents with source criteria and changed legacy rules.
- Local effects, verification limits, and the first Omni Grep commit are reported accurately.

## Verification And Review

The source-checkout command `python3 temp/engineering-transfer/check.py` passed for all 32 candidate files. It resolved local path links and checked added whitespace. Its negative control detected a missing local target. This temporary probe is retained in the source workspace, not installed as a project dependency.

An independent Codex reviewer inspected source criteria, legacy rules, candidate text, package scripts, and installed bytes. The reviewer found no actionable defect. Its separate path checks resolved 98 Proxy Gate links and 44 Omni Grep links. Review does not certify full ASD-STE100 compliance or product behavior.

`git diff --check` passed in Proxy Gate. `git diff --cached --check` passed in Omni Grep. A direct comparison confirmed retention of every existing Data Source and Cloud Gate local-rule line. The retained operational README sections matched their original text. A package-metadata probe confirmed all 19 focused command references and the absence of node-runtime scripts.

No product tests, build, deployment, or external service checks ran. The changed surface is documentation; Omni Grep has no product implementation. Documentation checks and independent criterion review are the applicable proof.

The primary remains the sole writer. Proxy Gate contains 18 changed or new documentation files. Omni Grep contains 13 Markdown files and one ignore file. The shared rules have nine responsibility owners: execution, contribution, principles, coordination, roles, documentation, verification, writing, and toolchain. Proxy Gate also retains three local application rule sets. No executable registry, runner, service, or durable product state was added.


## Omni Grep Initialization

The target directory was empty before this task. Its undefined product state is an explicit engineer constraint, not missing information to fill.

The baseline includes discovery, contribution procedure, engineering owners, and minimal ignore rules. The toolchain guide records the workspace's mise convention without selecting a runtime. The CLAUDE.md file points to the single root contract.

## Checkpoint

Git was initialized on `main`. The engineer authorized the first commit after review. This record contains pre-commit verification; Git records the commit outcome. No remote, product source, package manifest, CI workflow, or deployment was added.
