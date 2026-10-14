# Documentation Governance

This document owns document authority, lifecycle, and organization. It does not define product behavior.

## Assign One Owner

| Document | Responsibility |
| --- | --- |
| Root AGENTS.md | Repository execution and strict safety boundaries. |
| Engineering documents in this directory | The engineering concern stated in each document's opening. |
| Accepted product contracts | Intended behavior, architecture, interfaces, and durable lifecycle. |
| README and local guides | Discovery, current commands, and directory workflow. |
| Decision records | Who decided, why, alternatives, consequences, and revisit conditions. |
| Change and review records | Intent sources, observations, progress, and findings. |

Locate the owner before comparing rule strength. Within the same authority and scope, mandatory requirements take precedence over preferences. Correct a lower-authority summary when its owner is clear. Return conflicting owners or ambiguous intent to the engineer.

Documents record interpreted intent; they do not prove that the interpretation is faithful. Compare consequential rules with the source request and decisions. A plan, implementation, or later date cannot silently supersede an accepted contract.

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

## Keep The Corpus Small

Store each rule in one owner and link to it. Keep rationale separate from procedures and contracts. Record history in Git instead of copying revisions into active guidance.

Group documents by the concepts that already exist. Create a directory, type, index generator, or validator only when current use justifies it. A short manual map is sufficient for a small corpus.

Use repository-relative links. Each required link must resolve in this repository. General engineering rules must work without a checkout of their source project.

For a substantive rewrite, list each criterion moved, merged, changed, or removed with its destination or accepted reason. Obtain an independent comparison against the old text. A link check cannot detect lost meaning.

Update a changed behavior's owner before or with code. Update affected user guidance and local guides in the same change. An implementation-only change needs no normative rewrite.

## Preserve Existing Authority

Adopting this document does not automatically approve, reject, or demote existing product documents. Resolve their status from explicit acceptance evidence before changing their governing meaning. Preserve local architectural rules until their owner changes.

Plans, audit reports, and operating history remain evidence. Their command examples do not authorize a new effect. Future work follows current engineering governance.

Write engineering documentation in English. Existing localized operator manuals may remain in their current language. They retain their audience and operational constraints; they do not define repository engineering policy.
