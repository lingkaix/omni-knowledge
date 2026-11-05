# M1 Native Consumer Freeze

This guide describes the compatible correction of reviewed subject `24f1d9f3535f3d932b8858883a0099bd2bd2a31d`.
The [storage owner](../../docs/specs/storage.md) and [history owner](../../docs/specs/history.md) remain unchanged.
The exact committed correction and observed gates are published in the external M1 fix receipt, not asserted by this interface guide.

## Compatible Calls

`Store<P: WritePolicy>::open`, `Scope { claim, key, scratch_bytes }`, `Expected`, `Preflight`, `prepare`, `install`, `publish`, `inspect`, `apply`, `reconcile_local`, and `replay_tracking(offset, preflight)` retain their signatures.
Initialization, import, the held-source-FD boundary, and the local native backend retain their signatures.
Use the actual process `Account` and its queue, not another pool or a source-metadata permission approximation.
The `Scope` RustDoc contains compiled native publication usage with the real generic owner and existing claim/key types.
The ordinary native tests execute the actual filesystem and Git calls; the RustDoc is compile proof only.

`Claim::stop_requested(expected: Key<'_>) -> Result<Option<Cancellation>, JobError>` is additive.
It validates the exact queue-owned claim generation, complete key, and unsettled state without releasing anything.
It observes `Requested`, `Superseded`, or `Deadline`, and refuses stale or mismatched claims.
Storage returns additive `Error::Stopped(Cancellation)` or `Error::Claim(JobError)` before continuing controlled executor work.
Consumers with exhaustive error mapping must add these variants and map them to their existing cancelled, superseded, or stale execution outcome.
Do not settle a job until the actual children, I/O, and owned allocations have ended.
A synchronous split loan returns only after its actual buffers have been destroyed; stale returns stay charged.

## Current Admission And Work Planning

Every retained intent path is re-admitted against the current native `WritePolicy`, including proposal publication, before Git preparation or publication.
Exclusions, selected text root, raw-write approval, native root binding, and current revision are not replaced by immutable intent, matching digests, or manifest declarations.
Installation rechecks current parent boundaries, source bytes, policy, and live execution immediately before displacement.
The checks are cooperative boundaries, not preemption of an already running syscall or the separately owned Git child.

Preparation sizes the complete simultaneous reference, entry, escaped control, and clone costs before collection or sorting.
Sorting uses no secondary stable-sort allocation, and fingerprint serialization borrows the already normalized sequence.
Restored bodies use a separate bounded scratch portion and a decreasing aggregate encoded-byte allowance.
Native text-save work budgets include both candidate evidence and the independent install copy: require `2 * sum(candidate_lengths) <= max_bytes` with checked arithmetic.
Caller-owned request/result and Git costs remain in the parent's admitted remainder.
Recovery/home bytes are authored operation evidence, not generated-cache-volume promises; this is not a filesystem quota.

## Recovery And Tracking

New text-save `new-N` files remain independent from editable home inodes.
Only the selected operation's necessary content is copied through bounded scratch, checked against its actual selected native stage version and complete expected digest, synchronized, and installed by no-replace rename.
A failed copy remains visible, and an unknown copy is never deleted or adopted by a retry.
No whole-library snapshot or unconditional rollback is introduced.
A historical saved receipt can remain `SavedLocal` with `current_matches: false` after a normal in-place edit.
Previously damaged evidence cannot be recreated by declarations or silently repaired.

Inventory rebuild reads relevant qualified observations from every visible writer stream with a finite aggregate replay budget.
Each writer retains independent append and projection offsets; no cross-writer timestamp or sequence supplies causal authority.
Unchanged qualified native sources keep their asset IDs, equal-content sources remain distinct, tombstones are not resurrected by stale peer evidence, and contradictory mappings require recovery.
Current root, backend, mount, read policy, and identity ambiguity still constrain reconciliation.
A resumed byte cursor validates its bounded predecessor, including EOF; invalid, omitted, or reordered sequences stop with retained evidence instead of false completeness.
Root cold decoding bounds actual encoded reads before parsing, including a concurrently growing held FD, and the existing resident owner retains its memory charge through its last reader.
The retained non-search owned-snapshot caller still has its separately documented integration lifetime gap.

## Qualification

Linux runtime proof, native macOS proof, M6 caller composition, and independent affected-scope acceptance remain distinct gates.
The hosted macOS native-cache fixture loan covers only `crates/knowledge-core/tests/cache/support.rs` and `crates/knowledge-core/tests/cache/public_consumer.rs`; this correction does not edit those files or reopen their production owner.
