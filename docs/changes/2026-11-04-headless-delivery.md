# Headless-First Delivery Amendment

Date: 2026-11-04.
Status: Accepted engineer scope amendment; it records delivery priority, not implementation results.
Intent source: the engineer's headless-first scope amendment accepted on 2026-11-04.
This record explains the amendment, its owner destinations, criterion mapping, necessity, and evidence limits.
It is not a behavioral owner, implementation report, or release approval.
Current behavior is owned by [product](../specs/product.md) and the other `docs/specs/` owners.

## GUI Priority Correction

The engineer's later correction on 2026-11-04 refines this amendment: GUI milestone M7 has lower priority, not a stop, and its exit criteria stay in force for later delivery.
No new GPUI investment happens beyond keeping the existing application building until GUI technology is confirmed; a shared Web knowledge component is only a possible direction under discussion, not a selected technology or dependency.
The headless-first priority for the service, MCP, and CLI mechanisms stays in force, and this record keeps its original wording as the earlier decision history.
[Product](../specs/product.md) Delivery And Qualification owns the current status.

## Amendment And Boundaries

The amendment defers GUI milestone M7 and prioritizes the complete headless service, MCP, and CLI mechanisms in the current delivery.
The engineer will exercise the headless mechanisms through external agent clients before GUI feedback.
The existing GPUI application keeps building and adapts only where a service DTO change requires compatibility.
No GUI redesign is in scope, and the current headless delivery does not require GUI completion.
Full non-GUI mechanism requirements remain complete; they are not reduced to an MVP.
The deferral removes no legacy GUI behavior or test and claims no shipped future implementation.
This record stays inside the public corpus and carries no private context.

## Owner Destinations

[Product](../specs/product.md) Delivery And Qualification owns the delivery priority and the explicit deferral; that owner text is complete without this record.
The [full-version record](2026-11-04-full-version.md) gains the overriding-amendments row and the milestone-gate note.
[README](../../README.md) states the current priority in Full-Version Development and keeps the retained first-version instructions.
The [desktop guide](../gui.md) states the deferral and that it documents the retained application.
Service DTOs, storage, search, history, processing, and package owners are unchanged.

## Criterion Mapping

| Change | Old location and text | Destination and accepted reason |
| --- | --- | --- |
| Reworded | Product Outcome: "The full version is one delivery, not an MVP followed by deferred required features." | Product Outcome now separates the one-delivery rule from the 2026-11-04 GUI-milestone deferral and states that non-GUI requirements stay complete; the engineer's amendment is the accepted reason. |
| Extended | Product acceptance-sources line | Names the headless-first delivery amendment alongside the 2026-11-04 supervisor amendments. |
| Added | None | Product Delivery And Qualification: headless-first priority, explicit M7 deferral, retained GPUI build duty, and no GUI redesign. |
| Added | None | Full-version record: overriding-amendments row and milestone-gate note. |
| Added | None | README and desktop-guide status text with a link to this record. |
| Unchanged | Non-GUI mechanism requirements, legacy GUI behavior, GUI tests, DTOs, build instructions, and implementation-versus-contract warnings | Retained; this amendment changes delivery sequencing only. |

## Necessity And Ablation

Without this amendment, the delivery owners require GUI milestone completion inside the current delivery window, which contradicts the accepted priority.
The amendment records that engineer decision; it adds no registry, transport, protocol, mechanism, or dependency.
Removing it would restore the contradictory GUI-now requirement without changing any non-GUI criterion.

## Evidence Limits

This is a documentation-only amendment; changed production and test lines are both zero, and no DTO, manifest, lock, or code file changed.
No Rust, build, GUI, model, signing, archive, remote, or service result is claimed from documentation checks.
The gate covered committed-range whitespace, staged and unstaged whitespace, local Markdown links and anchors for the changed documents, and final committed-diff inspection.
Independent comparison of the criterion mapping against the old text remains required before acceptance; this writer cannot self-accept a governing amendment record.
