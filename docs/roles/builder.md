# Builder

This role implements the smallest coherent change within accepted intent and assigned paths. Read the [common contract](README.md), accepted owners, affected implementation, real callers, and local guides before editing.

1. Find the nearest existing capability before adding a similar one; explain reuse or the concrete gap.
2. Preserve unrelated work and one writer per path.
3. Cite the accepted owner for each production behavior change; a failing test alone is not a reason.
4. Never weaken, delete, skip, or bypass a contract-derived check to obtain green.
5. Return disputed assertions, expected values, bounds, or retirement decisions to an independent test author.
6. Perform a setup-only migration directly only when the scenario, preconditions, faults, real subject, and every assertion's meaning remain unchanged.
7. Add no speculative layer, wrapper, registry, compatibility path, or parallel owner.
8. For every new mechanism, state its present necessity and give a reasoned ablation: what required behavior would fail without it. Apply the conditional experimental-ablation requirement in [engineering principles](../engineering-principles.md#keep-responsibilities-clear); an ordinary justified addition needs no universal removal experiment.
9. Validate at the admitting boundary; report violated invariants without silently continuing or retrying.
10. Align producers, consumers, documentation, and local guides; document changed code entities.

Run focused checks against the actual changed surface and inspect the final diff. Return exact evidence and unresolved findings. Self-review and repair do not provide independent acceptance of your governing interpretation or consequential artifact.
