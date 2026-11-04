# Frozen Packages, Backup, And Migration

Status: Accepted full-version contract, 2026-11-04; no archive implementation or artifact is certified here.
This document owns native ZIP/ZIP64 snapshots, hostile-input validation, capture/import/restore, and foreign OpenKit v1 mapping.
[Storage](storage.md) owns mutable authority and legacy migration; [retrieval](search.md) owns searchable generation semantics.
[History](history.md) owns Git bundle admission and durable text history.
[Service](service.md) owns plan/result DTOs and user approval; [processing](processing.md) owns retained EPUB3 publication.

## One Native Format, Three Content Presets

Use one versioned native ZIP/ZIP64 format; presets select content, not different formats or converters.

| Preset | Content and limits |
| --- | --- |
| Frozen version | Selected core files/references and optional compatible complete indexes; opened read-only. |
| Backup snapshot | Selected core plus explicitly selected original bytes, optional text Git bundle and visible journal; never implied whole-NAS retention. |
| Migration package | Selected home/managed sources or external references for explicit destination mapping; no automatic root activation. |

A preview shows selected files/media, references, digest/processing coverage, estimated size, destination, warnings, and conflicts.
Require confirmation before large copying or remote-destination writes and explicit effect selection for sensitive-library export.
A reference snapshot records verified media digests and mount/path locations without copying multi-TB payloads.
Unhashed external sources remain explicitly uncovered metadata with null digest.
`require_complete` refuses until selected required hashing/coverage completes; it never invents a version ID.
Optional indexes cannot make unverified originals verified.
A backup can restore only bytes actually included; references cannot recreate missing NAS originals.

## Manifest And Inventory

The native root entries are `manifest.json`, optional `manifest.sig`, and the inventoried data entries.
The format identifier is `omni.knowledge-package` and version is `1`, distinct from mutable manifest version 2 and service wire version 1.
The manifest contains `format`, `version`, `library_id`, `name`, `text_root`, `preset`, `required_features`, `mounts`, `entries`, `external_references`, `indexes`, `history`, `journal`, `capture_window`, and `coverage`.
`text_root` preserves the chosen pages/wiki mapping; portable IDs remain data identities, not permissions.
`mounts` describes source IDs/names/backend/location facts without credentials or permission grants.
Each `entries` item contains normalized `path`, decimal-string `size_bytes`, lowercase SHA-256 `sha256`, `method` (`stored` or `deflate`), and `role` (`core`, `original`, `index`, `history`, or `journal`).
Every data entry is inventoried exactly once; neither the manifest nor detached signature inventories itself.
Original-byte entries have explicit mount/source/path mappings; paths are not flattened or inferred from equal hashes.
Each external reference binds `source_id`, `mount_id`, `path`, `digest_state`, nullable `digest`, `availability`, and known size/media facts.
Missing/unverified reference facts remain explicit, not fabricated by package selection.
`indexes` binds the complete lexical/vector/processor/model/dictionary/generation identities and inventory paths of its components.
`history`/`journal` are null when absent, otherwise identify their selected bundle/stream inventory paths and retained scope.
`capture_window` contains start/end observation timestamps, not a simultaneous-filesystem-snapshot claim.
`coverage` records selected versus omitted/unverified/pending/unavailable content and compatible index coverage, independently of file compression.
Unknown optional extension metadata survives inspection/re-export; unknown required features or format versions refuse installation.

Canonical manifest bytes use RFC 8785 JSON canonicalization; large byte values use the service decimal-string convention.
The snapshot ID is SHA-256 of those canonical bytes, not a hash of physical ZIP layout or a permission grant.
An optional detached `manifest.sig` signs that digest with the user's selected signing identity.
For SSH signing, use SSHSIG over the 32 decoded digest bytes with namespace `omni-knowledge.snapshot.v1`; verification trust comes from the user/system signer policy.
Report valid-known, valid-unknown, invalid, and unsigned separately.
A valid manifest signature authenticates the inventory claim, not unread payload bytes.
Exclude private keys, credentials/tokens, live absolute `.git` redirects, service control state, local approval grants, and provider model weights.
Public signing identity material is not a grant to impersonate its owner.

## Hostile Archive Validation

Verify structure before reading/installing destinations or contacting declared mounts.
Admit only UTF-8 normalized confined entry paths with `/` separators; reject absolute/drive/UNC paths, dot/traversal/empty segments, NUL, duplicates, and target-filesystem case collisions.
Reject ambiguous Unicode/path representations rather than installing names that normalize to the same destination.
Reject encrypted entries, links, devices/special files, unsupported required compression/semantics, and file/directory prefix collisions.
Directory entries, if present, carry no data and must agree with the regular-file inventory and confinement rules.
No ZIP entry can overwrite the manifest/signature namespace or escape through a pre-existing destination symlink.

Validate central-directory/local-header agreement, filename/flags/method consistency, ZIP64 lengths/offsets, local data extents, overlaps, and declared compression sizes.
Use checked arithmetic for offsets/lengths and actually support ranges above 4 GiB.
Reject header disagreement or wraparound before any mmap or read slice.
CRC is not a substitute for the SHA-256 inventory.
Apply finite archive-byte, entry-count, individual expanded-byte, total expanded-byte, compression-ratio, path-depth/length, memory, and processing-time budgets.
Quotas are explicit application/job settings; a quota refusal is not a corrupt-success downgrade.
Do not inflate an entire archive or trust an entry's declared length as a resource reservation.
Imported text/AGENTS/schema/scripts remain inert; archive inspection never executes hooks or grants service exposure.

## Per-Entry Storage And Direct Search

Media, exact-vector segments, and every prebuilt lexical/index component are STORED.
Authored text uses deflate; no whole-archive compression layer is added around the ZIP.
Central-directory lookup yields bounded `(offset,length,method)` entry slices.
For STORED entries, hold the immutable archive handle and use bounded ranges or aligned mmap slices.
Align a mapping down to OS granularity and slice only the admitted entry; optional ZIP-extra alignment padding is not required correctness machinery.
Deflated text streams/inflates on demand under bounds, optionally into external derived cache; it is not randomly mmap-searchable.

Use the kernel's generic read-only Directory/generation provider to query compatible STORED Tantivy components inside the archive.
An unmodified filesystem-only kernel cannot do this merely because entries are uncompressed.
Exact disk-backed float32 vectors are the required directly searchable packaged-vector representation.
If Lance is selected experimentally, export already-computed selected vectors into exact segments without rerunning embeddings.
Do not claim native Lance files are directly ZIP-readable without a proven version-specific archive IO adapter.
Optional incompatible/unsupported index generations are omitted or rebuilt externally with an explicit reason; core bytes remain usable.
No index is extracted into the authored home as a hidden workaround for a direct-search claim.

Pin the archive identity and generation for a query; detect/revalidate replacement or mutation of the underlying container.
Verify each index-entry digest before first query use and cache verification only for the unchanged container identity.
A small media range alone cannot authenticate its complete entry digest.
Strict whole-entry integrity verifies the entry before claiming that identity and may incur a first-read scan.
Expose manifest-only, selected-entry, or all-entry verification state/cost honestly.
No Merkle protocol is required, and a manifest signature does not excuse skipping payload verification.

## Capture, Installation, And Frozen Use

Stream and rehash selected entries while exporting; compare current source versions and reject detected drift.
Storage's stable-read rules still apply; arbitrary changing NAS files are not captured at one atomic instant.
Write to private output, finish the ZIP central directory, validate the completed artifact, then atomically install without overwrite.
Interrupted/private output is not a completed snapshot; retain/report cleanup state after forced interruption.
No-clobber includes destination races, not just an initial existence check.
Capture records selected references and coverage even when originals are intentionally absent.

A frozen package is read-only.
Opening does not activate packaged external roots, widen exposure, run Git, or modify the archive.
Reads of external references require fresh explicit local root mapping/admission and source verification.
Editing creates an explicit mutable home/working branch; never update a frozen ZIP in place.
Multiple readers pin immutable generation/container identities and cannot observe mixed versions after replacement.

## Import And Restore

Verify the package before touching a destination; default to a new home.
Produce a materialization plan with byte/path mappings, mount remaps, selected history/journal, conflicts, and uncovered references.
Machine-specific locations require explicit remapping and approval; importing a mount declaration never contacts it.
Installation uses storage's exclusive/no-clobber writes and preserved conflict versions.
Restore is not deletion synchronization and must not remove unrelated destination files to match a package.
A stale plan or changed input/destination refuses or reports conflict/indeterminate recovery instead of overwriting.
The operation/journal records recovery identity; a retry inspects actual results rather than blindly repeating installs.

Treat a Git bundle as inert history until `git bundle verify` and controlled import succeed.
Disable hooks/filters, remotes, submodule effects, and automatic network contact during validation/import.
Never overwrite an unrelated repository or claim omni Git is an OpenKit Notebook revision.
Restore history outside sync roots, separate from cache, with explicit identity/target selection.
Source annotations and orphan pointers survive even when referenced media cannot be restored.

## OpenKit v1 Directory/Tar Adapter

The existing external v1 package format is a compatibility adapter, not native ZIP and not an instruction to change OpenKit.
Import a genuine v1 directory/tar package under its actual inventory, path, digest, quota, and required-feature rules before projection.
Project pages, source facts, original/reference mappings, and parsed local links into the ungoverned core.
Foreign scope/schema/Review/history labels grant no local permission or approval.
Preserve ordinary authored extensions; unsupported required semantics refuse rather than silently disappear.
Raw digests and source/link mappings remain comparable modulo an explicit ID/reference map.
Parsed rewriting must not change prose, code examples, source bytes, or unrelated metadata.

Current v1 requires genuine foreign owner/revision/profile context that a new unowned library does not possess.
Export therefore needs genuine host-supplied context or host finalization for those required semantics.
Without it, return `compatibility_refused` plus a core mapping preview; native ZIP stays fully usable.
Never fabricate owner scope, approval, published revision, or Review to satisfy a schema.
A genuine-input round trip may retain foreign provenance only while its claimed meaning holds.
Changed content must not retain a false published-revision claim or call its ordinary Git commit a Notebook publication.
Equivalence tests compare core bytes/annotations/links/digest pins, not governance, receipts, or Git hash identity.
No mandatory OpenKit connection, ownership registry, scope-free reinterpretation of current v1, or freeze amendment is introduced here.

## EPUB Publication Boundary

EPUB3 remains an explicit service operation, not an alternate native snapshot preset or recovery format.
`epub_plan` selects ordered Markdown/HTML pages, optional explicit contained media, destination, and budget.
`epub_export` consumes that bound plan and produces navigation/chapters through the existing processor behavior.
Do not recursively copy media, resolve remote resources, or bypass exclusions/sensitive-export approval.
Validate output and use private staging plus atomic no-clobber installation; unsupported content remains a precise failure/warning.
Its terminal service result has no native snapshot ID or native manifest-signature claim.
The round-trip acceptance is readable navigation/chapters and selected media, not preservation of all publisher layout or native package history.

## Acceptance

Compose the actual native writer, archive reader, and kernel; search compatible STORED indexes with extraction disabled.
Prove ZIP64 by actually reading/writing sparse fixture offsets above 4 GiB, not only small integer unit cases.
Exercise duplicate/escaping/case-colliding paths, encrypted/special/link entries, hostile ratios, header disagreement, overlapping ranges, and invalid payload hashes/signatures.
Exercise immutable-reader pinning, archive replacement, strict first-read integrity, incompatible-index fallback, drift/interruption, destination races, and safe bundle admission.
Prove all three presets and reference-only large-library selection with explicit unverified coverage and require-complete refusal.
Restore selected originals/text/annotations into a new home and compare actual bytes/mappings; absent NAS references remain unavailable, not restored.
Test genuine v1 core import/mapping, truthful context refusal, and a genuinely finalized export separately when context is available.
Test EPUB export through the service and reopen its readable navigation/chapters rather than bypassing the frozen wire.
Synthetic sparse files prove arithmetic/range behavior, not measured multi-TB throughput.
