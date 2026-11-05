# Knowledge Storage

This crate owns home bytes, visible writer observations and inspectable same-filesystem recovery under the [storage owner](../../docs/specs/storage.md).
It consumes the existing process admission account and [local Git seam](../knowledge-history/README.md); it is neither authority nor another annotation/Git/resource engine.
Native authorized context comes from the actual service owner, never a manifest declaration, wire boolean or recovery record.
The [M1 native consumer freeze](m1-interface.md) gives compatible compiled calls and exact migration notes for the seven corrections of review subject `24f1d9f3535f3d932b8858883a0099bd2bd2a31d`.

## Durable Encodings And Safe Boundary

Visible `journal/<writer-id>.jsonl` records use `format_version: 1` with writer-local canonical decimal-string sequence numbers.
Every record contains writer_id, sequence, observation_id, nullable operation_id/source_id, nullable old_digest/new_digest and old_path/new_path, availability, evidence_kind and UTC observed_at.
Operation records additionally bind the immutable normalized intent digest, Git candidate and outcome.
No source bodies, query strings, raw arguments, credentials or signing keys are journal fields.
A truncated/malformed final record is preserved and reported; appending never silently repairs or discards it.

`journal/recovery/<operation-hash>/intent.json` is visible recovery evidence, not a second authored authority or permission store.
Its explicit version-1 encoding binds the operation, writer, normalized intent digest, selected root/policy revision, exact Git base/destination and each selected path's expected native version/new digest.
`prepared.json` retains the actual Git owner's immutable preparation; `begin.json` and `saved.json` contain writer, offset, length and SHA-256 proofs of exact synchronized visible journal records.
Inspection reconstructs the normalized candidate intent and compares actual Git facts; sidecar declarations alone cannot manufacture a save.
Exclusive text-save `new-N` files contain exact staged candidate bytes independently from editable home inodes; exclusive `old-N` names retain displaced originals on the SAME home filesystem.
Only the selected operation's candidate is copied to an independent install inode, then installed by no-replace rename; simultaneous copy costs are pre-admitted, and partial copies remain visible for inspection.
Unknown recovery files and contradictory facts require inspection, never deletion or adoption.
A successful save requires actual matching home bytes, actual Git publication and a synchronized visible final journal record; Git Published alone is not saved_local.

Prepare records visible intent before effects, stages exact candidates exclusively and prepares the immutable Git operation.
Install checks expected full SHA-256 plus native identity/stat, rechecks live claim and current policy immediately before displacement, moves the selected old entry into a no-clobber recovery name, rechecks the displaced bytes and installs the candidate exclusively.
Publication re-admits every intent path against current exclusions, selected text root and raw-write policy before Git/native effects, including proposals.
A competing destination is never overwritten, and no automatic rollback can overwrite a later writer.
An interrupted/conflicting operation retains original, competing and candidate versions with typed recovery facts.
Inspect observes facts without replay; resume/install/publish are explicit operations under fresh current admission.
The mechanism does not promise atomic multi-file/filesystem/Git publication or immutable behavior against a malicious same-UID directory mover.

## Current Native Owner And Costs

The selected root is bound by no-follow descriptor identity and rechecked; exclusions and child markers are boundaries, including malformed markers.
Raw-original mutation is disabled by default; explicit import copies to an exclusive destination and never deletes or overwrites its source.
An imported basename omni-knowledge.json becomes omni-knowledge.asset.json.
Initialization preflights all destinations, records intent and installs the manifest LAST; failed partial unmarked initialization is reported rather than cleaned broadly.
Unknown authored manifest/frontmatter extensions are not reserialized by the byte installer; callers supply only the explicitly approved delta.

Text operations borrow the existing jobs Claim and a matching Key from the SAME process Account.
An explicit scratch reservation is split from that claim before allocation and can rejoin only that exact still-live claim generation without releasing aggregate charge; caller-owned input/result and Git child/index/object costs remain covered by the parent's admitted costs.
Complete metadata costs are checked before collection, sorting or cloning, and duplicate inspection destroys the outer temporary buffers before returning the loan.
The exact claim generation/key is checked for cancellation and supersession between controlled reads and effects, without settling unfinished resources.
Initialization/import use Preflight with an actual BeforeVerification holding and finite Core Budget, without inventing a verified original or a source job key.
The caller retains that holding through owned template/source/result lifetimes.
Storage work ceilings, monotonic deadlines and native buffers are checked; holding drop never stands for child termination.
Authored home/recovery bytes are not misrepresented as generated cache-volume bytes; generated processing writers use the account's selected-volume promises separately.
No opaque parser/RSS/watchdog or physical filesystem quota guarantee is invented.
M6/root startup must establish Account before actual callers; no second empty pool is created on NotStarted.

## Initialization And Import Encoding

`init.json` and `import.json` use explicit format_version 1 and kind init/import, operation/library/writer/root identities, and selected files with path, nullable digest and exact length.
The import source_stamp is unverified native metadata only: device, inode, length, mtime seconds/nanoseconds, then ctime seconds/nanoseconds; it is never a fabricated verified digest.
Initialization stages exact supplied bytes in new-N files and validates supported marker bytes with Core without reserializing their metadata projection.
`stage_initialization` changes no home payload; initialize runs a newly staged operation, while resume_initialization explicitly verifies retained stages and installs the root manifest last.
`initialized.json` proves the final initialized journal record; this does not claim a Git commit or saved_local.
Import hashes actual copied bytes into verified.json, installs exclusively, and writes imported.json with actual destination Version and final journal proof.
The final visible import record binds the actual new path/digest; inspection checks it against current native inode/stat/bytes without copying again or associating a different original by digest.
Duplicate/partial initialization or import is an explicit conflict/inspection path, never implicit replay.
No original or unknown recovery artifact is deleted.

## Tracking And Verification

Native watches are hints, not content/version or absence evidence.
Startup/lost continuity/overflow triggers bounded current-source reconciliation, and only a complete successful relevant scan may infer deletion.
Qualified birth/generation/volume identity can support continuity; equal digests, inode alone or restored mtime cannot merge assets or establish absence.
Previous complete generations and live pins remain the cache owner's responsibility; this module never evicts them to obtain capacity.
Builder checks use actual private files, competing native destinations, interrupted/reopened operations and real Git; they are not independent admission/durability acceptance.
The local adapter supplies admitted full-source verification, admitted stat-first enumeration and the shared inotify/FSEvents watch interface.
Store::reconcile_watched consumes the exact watch-owned mount and appends an optional typed event_cursor only after its complete inventory checkpoint is synchronized and current root, scope, and hint generation still agree.
The event_cursor contains root, backend/library/mount, read_scope, scan, native volume UUID, and device-relative event ID.
Old records remain readable; older binaries that reject the new optional field must not be used as event-cursor readers.
Replay::event_cursors exposes these retained observations, not native replay permission.
Restart always opens a fresh stream and performs real bounded catch-up with known-descendant inspection; no persisted cursor authorizes pruning.
A post-scan watch failure can return an error while the already synchronized inventory remains visible; no rollback is implied.
## Durable Inventory And Replay

Store::reconcile_local streams eager metadata with finite working memory, optionally verifies actual bodies, and appends version-1 observations to the selected writer journal.
Observations contain source_id, qualified backend/native root, actual stat and nullable verified Version, separate Verification pending/budget/failed/unsupported outcomes, change class and identity ambiguity.
Equal content never merges source IDs; rename continuity requires qualified reuse-resistant identity, absent old path and no hard-link ambiguity.
Complete scoped scans append durable Checkpoint records with actual read-policy revision/exclusion fingerprint, UTC time and explicit startup/change/quiet/daily/manual Schedule.
Partial or failed scans preserve observations and never infer absence from quiet, denial, unavailable storage or omitted coverage.
Checkpoint fingerprints and declared scopes are correlation evidence, not approvals.
M6 owns when caller-driven checks run; no background scheduler or event-replay claim is invented.

The external selected-volume projection is disposable and qualified by root, mount/backend and writer.
Reconciliation rebuilds relevant identity evidence from all visible writer streams, preserves unchanged asset IDs across sequential writers, and refuses contradictory mappings without comparing cross-writer sequences.
Every reused projection frame binds an exact actual visible journal proof; rebuild replays bounded lines sequentially without retaining whole inventory in RAM.
The generated namespace also binds the bounded validated first record of the actual visible writer stream, so native root-inode reuse cannot select an earlier stream's cache frames.
An empty stream receives a fresh disposable projection generation; later scans rebuild from its actual visible records, not from the old generated cache.
Old generated generations remain cache-owned and are not deleted or promoted to authored history by reconciliation.
Eviction does not lose source identities or annotation authority; replay reports exact byte cursors, bounded progress and malformed tails without truncation.
Resumed replay validates the bounded predecessor and writer-local sequence, including at EOF, before reporting progress or completion.
Large histories require explicit sufficient replay/work budgets; budget refusal is not a corpus-size prohibition or empty success.
Generated projection bytes reserve and reconcile through the same Account before publication.
Generated state must be outside both the home and selected source tree; actual descriptor ancestry is checked.

## Retained Root Adapter And Migration

Root storage.rs now routes JSON writes through native expected-version/no-clobber installation, and resident snapshots retain same-account memory through the last Arc reader.
Cold manifest decoding caps actual held-FD consumption at its pre-admitted encoded extent before parsing, even when an in-place writer grows the file.
Actual index builds reserve complete working memory and generated-volume promises before construction, and reconcile completed output bytes.
Enclosing builds lend their actual holding to JSON writes rather than double-charge the same generated output.
Authored metadata recovery remains visible; derived index JSON recovery is outside the home on the selected same native volume.
The retained v1 index layout is not silently converted, and auxiliary JSON publication does not claim v2 Git saved_local.
Read-only migration_inventory reports Core marker interpretation, pages/wiki conflicts, known legacy indexes/sidecars and unknown preserved state.
It performs no migration, deletion, secret lookup, configuration/account action or unknown-state adoption.

The composed Git seam includes correction 2c86db624bc705343e51483b70f087588b29ad4a; the native writer's earlier frozen seam remains historical evidence.
Native FSEvents now has an explicit locked macOS binding and a real native-only delivery test.
Native macOS compilation/execution and consequential independent review remain qualification gaps, not Linux-derived successes.
See the [M1 change record](../../docs/changes/2026-11-05-m1-storage.md) for exact gates, remaining integration ownership, historical ENOSPC receipts and later actual positive Linux event/resource-lifetime passes without limit changes.
