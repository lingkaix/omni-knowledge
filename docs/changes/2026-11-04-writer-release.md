# Completed Operation Writer Release

Date: 2026-11-04.
Status: implementation and targeted proof complete; final rebased-head gate and integration batch remain pending.
The unchanged independent public-API probe fails on OLD and passes on the corrected library, including active-writer refusal and pre-exec child synchronization.
Existing file-action checks pass 11 tests and the knowledge-base suite passes 29 tests; strict workspace Clippy and formatting pass.
These observations do not claim a complete deterministic workspace batch.
Owner: [storage mutations](../specs/storage.md), cooperative exclusion while an operation is active.
This correction does not change the accepted write contract or authorize weaker exclusion.

## Defect And Independent Observation

A completed library operation can leave its cooperative writer lock held by an unrelated child suspended between fork and exec.
On Unix, closing the parent's file does not release an open-file-description lock while the child retains the inherited description.
A subsequent public metadata write can therefore report an active writer after the real operation has ended.
Default parallel baseline tests exposed intermittent writer-busy failures; running tests serially only hid the interaction.

An independent test author reproduced the defect through public `index` and `set_metadata` operations on the old implementation.
A controlled external provider response keeps indexing active while a disposable unrelated child reaches a pre-exec barrier.
The test observes refusal during indexing, completes the failed index, and then requires the metadata write to succeed before releasing the child.
It releases and reaps the child before deciding assertions and also checks a write after child exit.
The provider is the only external-effect double; the library, filesystem, writer, and child process are real.
The original failing observations remain evidence; a later passing result does not erase them.

## Smallest Correction And Ablation

Give the existing operation lock a private RAII guard that explicitly unlocks before closing its file.
Create the guard immediately after acquisition so subsequent setup failures also release ownership.
No caller learns a new ordering or cleanup step; acquisition, competing-writer refusal, and protected operation scope remain unchanged.
There is no new lock file, registry, protocol, retry loop, process-global mutex, or dependency.

The guard is necessary because a bare file close does not end ownership in the demonstrated inherited-description case.
Removing explicit unlock restores the independently observed failed release assertion while preserving the active-writer precondition.
The already observed old implementation supplies this ablation; no additional removal experiment is needed.
This is a correction to still-required behavior, not a coupled-test replacement or intentional compatibility change.
No existing test is deleted or weakened.

## Verification Boundary

Run the independently authored public lifetime regression on the correction, plus existing writer exclusion, transfer recovery, metadata, and publication checks.
Run formatting, strict Clippy, and the full deterministic workspace suite at the integration batch.
Gate the final rebased commit before pushing and retain exact commands, code identities, outcomes, and declared platform limits.
The Unix process regression proves the exercised fork/exec case; it does not certify cloud synchronization or power-loss durability.
