# Native readonly inventory and precise write-boundary controls

Scope: existing M1 replay and its native Query adapter; no whole-product activation or READY claim.

## Necessity and minimal path

A selected readonly Query must not create/reconcile a journal, borrow a Refresh, or manufacture `writable=true`. `Store::replay_tracking_readonly` uses the SAME bounded decoder/cursor/issue machinery, actual LocalMount read policy, matching Store/root FDs and repeated current read-context checks. The old write-gated API is unchanged. No second journal, index, registry or resource engine is introduced.

`NativeLibraryService::inventory_for_query` borrows the actual Query Permit; its caller keeps that owner through paging/delivery. Root/policy/context-copy admission is shared with actual Refresh, with explicit sealed command and selected policy checks. Existing checkpoint evidence must establish the supplied complete scan under that actual root/backend. Replay has total byte/record/deadline limits in addition to bounded pages. Labels are DATA, not authority. Context and scratch are paid before inspection. No model selection or source-byte/transmission authority is asserted.

Native page replay now uses the actual readonly primitive. The Query binding retains actual current SelectionLease/read policy, and all page/data/FD owners retain their existing credit. InventoryPolicy does not capture Permit (avoids the existing source-check→inventory→Permit cycle); the operation controller retains the separate actual Query owner.

## Deciding controls

- Storage matching-readonly/no-tree-change, partial cursor, old-write-API refusal, denied/revoked/changing read policy, foreign same-label root and missing lane/no-creation controls pass.
- Removing actual matching root-FD comparison fails one actual test. Removing both read-scope fingerprints initially SURVIVED because root_identity independently checks permission; that is not necessity evidence for the fingerprints. The stronger still-Admit-but-changed-scope case fails under that ablation. Both checks restored; full Storage passes.
- Actual readonly SDK policy + actual Query binds existing complete inventory, preserves the durable tree, refuses Status and an invented scan, and observes actual owner revocation. OLD native page using write-gated replay fails `storage Context`; restored readonly page passes. Starting checkpoint-found as true fails the invented-scan control. Both ablations restored.
- An initial native fixture allocated before account startup and failed `NotStarted`; fixture setup corrected, not a production repair.

## Actual native CI and write oracle

Public580 actual run42205437264: Linux696 succeeds; macOS658 passed, 2 failed, 1 declared ASR opt-in; strict not reached. Existing compatible-provider and forced inherited-socket cases both pass on macOS. The new corpus waiting child failed the old absolute-zero premise after15,336 earlier bytes; later vector children were not executed.

The explicit paid-progress Linux variant leaves64KiB available, legitimately appends61,155 bytes and fails the same old zero premise. Replacement checks each actual paid tape/source/catalog append and no further append after visible Waiting/cancel. `WriteAdmitted` carries numeric existing-credit DATA only, never grants. The source/Claim/destination are rechecked after the observer callback; actual Core growth and all ceilings are unchanged. NEW paid-progress case passes; whole changed-tree/native gates are still required.

The separate History initial-commit `Io` is not diagnosed or repaired by these controls. Optional `OMNI_NATIVE_TRACE` now records fixed native I/O stages and numeric kind/errno without paths, arguments or Git stderr. Error classifications, collection/retention, effects, retries and original64MiB bound remain unchanged. A later native pass must not be reported as the old cause's diagnosis.

Whole tuple publication/recovery/retirement, complete unavailable/tombstone coverage, bounded application startup/query activation, actual WeMM/quality and M5/M6 full-product holds remain open.
