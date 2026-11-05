# Adopted Resource Accounting And Integer Syntax

## Authority And Plan

The engineer's 00:35Z decisions approve qualification of necessary M1 storage/cache/admission, M3 processing and M4 retrieval against actual CLI/MCP/library entrypoints before M0 acceptance.
M0 criteria stay unchanged and durable-authority activation remains separately gated.
Use one shared admission/reservation seam; do not build a second bounded-v1 engine, introduce a corpus ceiling or silently disable a processor.

Storage now owns the simple process-wide memory default, job-attributed child/runtime/model/pinned costs, visible waiting-for-memory behavior, typed over-whole-budget refusal and generated-storage reservation floor.
Service references that single policy and requires effective budgets in status/MCP with configuration overrides.
The disk percentage is expressed against the cache volume capacity; accounting does not substitute for physical processor enforcement or actual worker settlement.
Existing format-specific hostile-input ceilings are unchanged.

The engineer also resolves the prior lexical question: safe-integer fields admit canonical JSON integer tokens only, refusing exponent, fractional and negative-zero spellings.
The existing `1e2` refusal is now normative; `100.0` and `-0` require independent public-codec evidence alongside canonical preservation controls.
This rule does not ban genuine non-integer score fields or change existing decimal-string encodings.
No wire version/field shape or enum is changed by this owner clarification.

## Implementation And Verification Boundary

This commit changes owners/documentation only; it does not install defaults, enforce resources, expose status fields, change the codec or assert platform acceptance.
Independent tests and consequential review remain required for implementation and actual entrypoint cutover.
The existing private 96e8f12 workspace receipt belongs to that exact prior head, not this moving documentation subject.
Mac diagnostics may add only the separately approved bounded numeric self-VM observation at the actual owned worker/resource-install boundary, retaining every limit, order and fail-closed outcome.
If an opaque Mac processor cannot be bounded, report an explicit replacement/capability decision rather than quietly defer it.

## Necessity And Alternatives

A single shared policy prevents per-caller pools from multiplying available memory and prevents releasing resource charges merely because a worker returned while pins remain.
Simple fixed defaults and explicit overrides avoid an invented corpus-size cap or policy registry.
Canonical integer tokens resolve the documented interoperability ambiguity without a second JSON grammar/parser.
These are approved owner choices; no empirical ablation or successful enforcement measurement is claimed by a documentation commit.
