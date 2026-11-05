# Knowledge Storage

This crate owns home bytes, visible writer observations and inspectable same-filesystem recovery under the [storage owner](../../docs/specs/storage.md).
It consumes the existing process admission account and [local Git seam](../knowledge-history/README.md); it is neither authority nor another annotation/Git/resource engine.
Native authorized context comes from the actual service owner, never a manifest declaration, wire boolean or recovery record.

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
Exclusive `new-N` files contain exact staged candidate bytes; exclusive `old-N` names retain displaced originals on the SAME home filesystem.
Unknown recovery files and contradictory facts require inspection, never deletion or adoption.
A successful save requires actual matching home bytes, actual Git publication and a synchronized visible final journal record; Git Published alone is not saved_local.

Prepare records visible intent before effects, stages exact candidates exclusively and prepares the immutable Git operation.
Install checks expected full SHA-256 plus native identity/stat, moves the selected old entry into a no-clobber recovery name, rechecks the displaced bytes and installs the candidate exclusively.
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
The local adapter now supplies admitted full-source verification, admitted stat-first enumeration and caller-driven Linux inotify hints.
Durable inventory/event replay, legacy migration inventory and Root storage.rs cutover are unfinished; this is a coherent native writer boundary, not complete M1 delivery.
Native macOS execution and unavailable platform compilation remain separate qualification gaps; FSEvents is not yet implemented.
See the [M1 change record](../../docs/changes/2026-11-05-m1-storage.md) for exact gates and the retained positive-watch ENOSPC setup RED.
