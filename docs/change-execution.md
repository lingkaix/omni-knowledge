# Change Execution

This document owns work coordination and change records. It does not authorize product design or external effects.

## Choose A Proportionate Method

Ordinary scoped work can proceed directly. Material work needs enough coordination to control consequence, uncertainty, reversibility, and shared ownership.

Use a record under `docs/changes/` when context must survive a handoff, a long task, or an engineer-requested plan. Do not create a record for every edit.

Record the source request, outcome, constraints, acceptance observations, exclusions, effect boundary, and owners. Separate engineer decisions from agent inferences. Append changes to intent with their source and reason. Keep current facts, unknowns, method, and next action in a replaceable checkpoint.

A record is evidence. It cannot supply missing design authority. On resume, reconcile its claims with the actual files, Git state, and named observations.

## Execute The Next Useful Step

1. Read the owner, affected implementation, callers, and local guides.
2. State the unresolved question or expected behavior.
3. Choose the cheapest safe probe or sufficient regression.
4. Make one coherent change within the accepted scope.
5. Inspect the result and run focused checks.
6. Correct locally, revise the method, ask for a decision, or close.

Before a costly, delegated, repeated, or effectful action, state what artifact, belief, or decision should change. Name the observation that would defeat the route. Afterward, record what happened.

Methods, test order, and task decomposition may change within accepted intent. Changes to intent, architecture, governing trade-offs, strict risk, or reserved gates need the engineer.

When progress stops, inspect the premise before repeating the method. Obtain a fresh independent view when context loss, repeated failure, or a changed direction makes the current reasoning unreliable. Before renewed material investment after such a reframe, reconcile the new route with source intent.

If a useful probe or independent intervention leaves no credible route, ask the engineer. State the unresolved choice, evidence, alternatives, and a recommendation when supported. Pause only dependent work.

## Delegate By Responsibility

The primary agent owns integration and progress. Use the [role contracts](agent-roles.md) when a separate context adds capacity or needed independence. Roles are capabilities, not a required sequence.

Dispatch along module or document ownership boundaries. State the task, source intent, accepted owners, exact inputs, writable paths, prohibited effects, completion evidence, and return format. Assign one writer per path. Stop the old writer before transferring ownership.

Name interfaces that cross owners. For representation changes, derive checks from both sides of the boundary. Assign independent test design when producer bias or boundary uncertainty could hide a defect.

Use a model capable of the assigned judgment. A reviewer must be at least as capable as the producer. Use runtime restrictions where available; a prompt restriction alone is not containment. Keep model identifiers and machine-specific settings outside general engineering rules.

Never prescribe a passing verdict, conceal objections, or replace a disagreeing reviewer to obtain agreement. A report cannot substitute for inspecting the artifact.

## Retain Decisions And Findings

Put current rules in their owners. Put durable decisions and reasons in `docs/decisions/` when there is a real decision to preserve. Do not create empty document collections in advance.

For an unresolved finding, record its impact, evidence, owner, next action, and current status. A deferred finding needs a receiver and an activation condition. A closed finding needs closure evidence.

Keep raw output and experiments under untracked `temp/`. Before a pause or closeout, move useful conclusions into the appropriate owner or change record. Remove credentials and private data first. Untracked does not mean safe to delete without authority.

## Close With Evidence

Recompute verification from the final diff. Report the changed behavior or documents, exact commands and results, review findings, external effects, cleanup, and remaining uncertainty.

Inspect actual files or named execution output before acceptance. A producer can repair its work but cannot provide independent acceptance of its own governing document or consequential result. Use a separate reviewer for that judgment.

Keep records accurate: planned, active, blocked, and complete describe actual work. Report implementation completion separately from engineer approval or a reserved release gate.
