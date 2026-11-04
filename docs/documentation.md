# Documentation Governance

This document owns document authority, lifecycle, and organization. It does not define product behavior.

## Assign One Owner

| Document | Responsibility |
| --- | --- |
| Root AGENTS.md | Repository execution and strict safety boundaries. |
| Engineering documents in this directory | The engineering concern stated in each document's opening. |
| Role contracts in `docs/roles/` | Role-specific operation under change execution; the README owns common limits. |
| Test strategy in `docs/specs/` | Test boundaries and lifecycle; verification owns evidence quality. |
| Execution rationale | Reasons and alternatives, never an independent execution rule. |
| Accepted product contracts | Intended behavior, architecture, interfaces, and durable lifecycle. |
| README and local guides | Discovery, current commands, and directory workflow. |
| Decision records | Who decided, why, alternatives, consequences, and revisit conditions. |
| Change and review records | Intent sources, observations, progress, and findings. |

Locate the owner before comparing rule strength. Within the same authority and scope, mandatory requirements take precedence over preferences. Correct a lower-authority summary when its owner is clear. Return conflicting owners or ambiguous intent to the engineer.

Documents record interpreted intent; they do not prove that the interpretation is faithful. Compare consequential rules with the source request and decisions. A plan, implementation, or later date cannot silently supersede an accepted contract. Role contracts rank below the governance they apply; correct a conflicting role contract rather than inverting authority. A guide's summary of a product contract remains a projection, not a new behavioral owner. No product contract may depend on a removable change record to supply its current meaning.

## Write A Contract When Needed

For a new governing product decision, use a focused specification under `docs/specs/` when that decision arises. Mark it Draft until the engineer accepts it. Record the acceptance source and date without inventing approval.

A contract must state the following where applicable:

1. Definition, exclusions, and intended outcome.
2. The unique authority for durable data and the boundary of derived views.
3. Creation, update, termination, retry, and recovery behavior.
4. Conflict, missing data, stale data, restart, and dependency failure behavior.
5. Observable acceptance criteria, including important negative examples.

State why a class does not apply. Define boundary preconditions, postconditions, and invariants. Keep replaceable implementation choices outside the contract unless they protect a governing property.

Track acceptance and implementation separately. Accepted means the decision governs; it does not mean the feature exists. When replacing a contract, name the successor and preserve the reason and migration constraints.

## Rebuild From Admitted Contracts

The documentation test is whether a fresh implementation can meet accepted behavior without the old implementation source. Use accepted documents plus explicitly owned declarative boundary schemas and standalone conformance fixtures. Generated boundary schemas qualify only when their contract ownership is explicit. Admit database definitions and retained fixtures only where needed to read supported saved data. Do not admit generators, whole packages, or implementation source merely because rebuilding would otherwise be harder. This rule creates no new retained-format generator or automated rebuild gate. Accepted prose still owns authority, lifecycle, failure, and recovery semantics.

A sampled rebuild probe receives one bounded subsystem's admitted corpus and uses its existing public conformance observations. Each necessary question or guess reveals a documentation gap; an unusable implementation-specific criterion reveals coupling. Record the result as evidence, not as a replacement contract or universal approval.

## Keep The Corpus Small

Store each rule in one owner and link to it. Keep rationale separate from procedures and contracts. Record history in Git instead of copying revisions into active guidance.

Group documents by the concepts that already exist. Create a directory, type, index generator, or validator only when current use justifies it. A short manual map is sufficient for a small corpus.

Use repository-relative links. Each required link must resolve in this repository. General engineering rules must work without a checkout of their source project.

For a substantive rewrite, list each criterion moved, merged, changed, or removed with its old location and destination or accepted reason. Obtain an independent comparison against the old text and actual diff. A link check cannot detect lost meaning. The [writer contract](roles/writer.md) applies to documentation-led work. If an execution restriction prevents independent review, report that missing judgment; do not relabel self-review as acceptance.

Update a changed behavior's owner before or with code. Update affected user guidance and local guides in the same change. An implementation-only change needs no normative rewrite.

## Preserve Existing Authority

Adopting this document does not automatically approve, reject, or demote existing product documents. Resolve their status from explicit acceptance evidence before changing their governing meaning. Preserve local architectural rules until their owner changes.

Plans, audit reports, and operating history remain evidence. Their command examples do not authorize a new effect. Future work follows current engineering governance.

Write engineering documentation in English. Existing localized operator manuals may remain in their current language, subject to the root language rule. They retain their audience and operational constraints; they do not define repository engineering policy. For new translations, keep an identified canonical English page; a translation is its projection and does not define independent authority. The root language rule also applies.

Add document categories only for responsibilities that already exist. A directory or metadata field needs a current retrieval, lifecycle, or validation purpose, not a prediction of future members. Do not import another repository's filename enumeration, frontmatter schema, generated index, or validators without demonstrated local need.
