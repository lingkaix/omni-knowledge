# Change Execution

This document owns work coordination and change records. It does not authorize product design or external effects.

## Choose A Proportionate Method

Ordinary scoped work can proceed directly. Material work needs enough coordination to control consequence, uncertainty, reversibility, and shared ownership.

Use a record under `docs/changes/` when context must survive a handoff, a long task, or an engineer-requested plan. Do not create a record for every edit.

Record the source request, outcome, constraints, acceptance observations, exclusions, effect boundary, and owners. Separate engineer decisions from agent inferences. Preserve recorded intent revisions without rewriting or deleting them; append changes with their source and reason. Keep current facts, unknowns, method, and next action in a replaceable checkpoint.

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

When progress stops, inspect the premise before repeating the method. Obtain a fresh independent view when context loss, repeated failure, or a changed direction makes the current reasoning unreliable.

For material long-running work, obtain a fresh independent direction check after a reframe, long pause and resume, or primary replacement before renewed material investment. When the primary has compacted since its last independent direction check, obtain one before a direction-bearing commitment. Such commitments include changed scope or accepted design, substantial investment in a new route, strict external effects, or materially changed closure. An ordinary commit within an examined route is not itself a trigger. A meaningful evidence or stage trigger may be selected for the next independent direction check; once selected, reaching it requires that check before dependent work continues, even when the primary sees no drift. Do not add timers or activity quotas. Self-rereading does not establish independent recovery.

If a useful probe or independent intervention leaves no credible route, ask the engineer. State the unresolved choice, evidence, alternatives, and a recommendation when supported. Pause only dependent work.

## Delegate By Responsibility

The primary agent owns integration and progress. Use the [role contracts](roles/README.md) when a separate context adds capacity or needed independence. Roles are capabilities, not a required sequence.

Dispatch along module or document ownership boundaries. State the task, source intent, accepted owners, exact inputs, writable paths, prohibited effects, completion evidence, and return format. Assign one writer per path. Stop the old writer before transferring ownership.

Name interfaces that cross owners. For representation changes, derive checks from both sides of the boundary and assign an independent test author who reads both owners. As a stronger local addition, important implementation work needs an independent test author working from the specification and public interface, not the candidate implementation. [Testing lifecycle](specs/test-strategy.md#test-authority-and-lifecycle) governs failure triage, setup-only migration, test defects, and retirement. Builders may migrate setup mechanically without per-case oracle requests only when all stated scenario and assertion semantics remain unchanged.

Use a model capable of the assigned judgment. A reviewer must be at least as capable as the producer. Use runtime restrictions where available; a prompt restriction alone is not containment. Keep model identifiers and machine-specific settings outside general engineering rules.

Never prescribe a passing verdict, conceal objections, or replace a disagreeing reviewer to obtain agreement. A report cannot substitute for inspecting the artifact.

## Land And Check The Final Head

Keep each landing coherent and small enough for focused proof. As a local addition, run changed tests and directly related tests at each code landing, then run the complete suite in accounted batches where needed. The accounting invariants in [test strategy](specs/test-strategy.md#keep-coverage-and-cost-honest) derive from the source's release-gate rules; this generic worker cadence and application to local batches are local additions, not ports of that cadence. For documentation-only work, inspect meaning, retained criteria, and local references; run the applicable documentation and format checks. Do not require product runtime evidence for a content-only change or claim that content checks prove product behavior.

Recompute the gate from the actual final diff after integration and rebase. As a local addition, before pushing, gate the final HEAD, including the repository-wide check required by the changed surface. Record the reviewed base after integration and run `git diff --check <reviewed-base>..HEAD` over that committed range. Check staged changes with `git diff --cached --check` and unstaged changes with `git diff --check` separately; neither substitutes for the committed-range check. Any subsequent content change, conflict resolution, or rebase invalidates the affected evidence and requires that gate again. Retain exact HEAD identity, commands, outcomes, exclusions, and blocked checks. [Toolchain](toolchain.md) lists Cargo and documentation commands; no earlier head's green result substitutes for this gate.

Local additions: prefer fast, forward landings after sufficient evidence, not repeated broad gates without a changed question. A red result remains red until its cause is corrected; fix forward and rerun the affected gate without weakening its oracle. Do not retry to erase a measured result. A separately assigned post-landing quality assurance pass reports S1 critical, S2 material, and S3 minor findings under the [reviewer contract](roles/reviewer.md). That pass does not replace required pre-push checks or independent acceptance of governing documents. Strict safety may require pausing affected effects and seeking the engineer's decision instead of ordinary fix-forward work.

## Retain Decisions And Findings

Put current rules in their owners. Put durable decisions and reasons in `docs/decisions/` when there is a real decision to preserve. Do not create empty document collections in advance.

For an unresolved finding, record its impact, evidence, owner, next action, and current status. A deferred finding needs a receiver and an activation condition. A closed finding needs closure evidence.

Keep raw output and experiments under untracked `temp/`. Before a pause or closeout, move useful conclusions into the appropriate owner or change record. Remove credentials and private data first. Untracked does not mean safe to delete without authority.

## Close With Evidence

Recompute verification from the final diff. Report changed behavior or documents, exact commands and results, review findings, external effects, cleanup, and remaining uncertainty. Report changed test and production lines, files and owners touched, and any concepts, registries, or parallel copies added. Use raw numbers as explanatory signals, never targets or line limits. For maintenance, register the simplification hypothesis and compare old and new behavior with the same observation before accepting removal. Keep behavior-preserving maintenance separate from behavior changes.

Inspect actual files or named execution output before acceptance. A producer can repair its work but cannot provide independent acceptance of its own governing document or consequential result. Use a separate reviewer for that judgment.

Keep records accurate: planned, active, blocked, and complete describe actual work. Report implementation completion separately from engineer approval or a reserved release gate.

## Risk-Based Review

The engineer direction of 2026-11-05 01:05 adopts process-review rules R1, R4, and R5 for this repository and overrides the broader ordinary-review chain above where they conflict.

Independent review remains required for durability, including no-clobber apply, recovery, and Git transactions; for the hostile ZIP parser; for MCP or HTTP authority and exposure; and for the admission or reservation seam. Ordinary implementation, test correction, and documentation use builder tests, the maintained gate, and a forward landing. A separate test-author agent, an OLD/NEW frozen carrier, and another review round are not required for that ordinary work. A newly unexplained relevant failure still blocks its slice.

A coordinator is required only for an ownership conflict or abandoned claim, a change outside agreed scope or to a shared contract, extra hosted budget or another scarce shared resource, or an unresolved relevant failure that needs an exception. Routine promotion and permission relays are not required. Ordinary reports stay within six lines. The coordinator cannot waive a security, durability, or retained-format guarantee.
