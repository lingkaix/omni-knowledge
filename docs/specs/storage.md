# Library Storage And Recovery

Status: Accepted full-version contract, 2026-11-04; implementation status is separate.
This document owns mutable layout, mount/asset identity, observations, tracking, safe writes, and external cache lifecycle.
[Product](product.md) owns scope; [service](service.md) owns wire DTOs and exposure.
[History](history.md) owns text Git and remote effects; [packages](package.md) owns frozen interchange.
This replaces the legacy version-1 required-marker, hidden-sidecar, in-home-index, manual-only tracking contract.

## Visible Authority And Layout

Original files, authored text, manifest data, annotations, and corrected transcripts are authoritative.
Derived extraction, generated ASR, embeddings, previews, and indexes are rebuildable and never own annotations.
Opening an ordinary unmarked folder is permitted without creating files; persistence actions create the manifest with normal defaults.
A read-only folder can be a reference mount under a different writable home.
No account, OpenKit scope/schema/Policy, or hand-written configuration is required.

```text
home/
  omni-knowledge.json
  pages/                    # or the selected existing wiki/, never two authored trees
    index.md
    log.md                  # authored knowledge log, not runtime progress
  sources/<source-id>.md
  AGENTS.md
  schema.yaml
  journal/<writer-id>.jsonl
```

New libraries choose `pages/`; an existing `wiki/` can remain the mapped text root.
Unknown files remain visible; ambiguous roots or colliding destinations require a user choice.
Root `AGENTS.md` and `schema.yaml` are inert editing/content conventions, not executable instructions or service grants.
Keep foundational original snapshots, reference concepts, and provenance without claiming third-party content is relicensed.
`raw/<mount-name>/<relative-path>` is a logical address, not a required physical raw tree or symlink forest.
An optional app-managed source directory may be `raw/managed/`; external mount directory structure stays unchanged.
Attaching even a multi-TB mount never implies copying, flattening, or symlinking its originals into the home.
A readable mount name is unique within the library; renaming it records an alias map and does not change source IDs.
Home-relative files, including legacy raw assets, use a null mount ID and retain their home-relative logical path.
Mounted originals use their stable non-null mount ID and logical `raw/<mount-name>/<relative-path>` address.

The mutable manifest has `version: 2`, `library_id`, `name`, `text_root`, `mounts`, `exposure`, and `convention_version`.
`text_root` is `pages` or `wiki`; library/source/mount IDs are stable data identities, not permissions.
Each mount declaration contains `mount_id`, `name`, `backend`, `location`, and the intended read/write and reconciliation settings.
Manifest exposure describes intended mode, sensitivity, exclusions, and allowed transports; local service approval remains the upper bound.
Provider settings, model weights, credentials, tokens, signing keys, backend authorization, and OpenKit owner scopes do not belong in this data manifest.
Unknown optional authored extension fields survive round trips; unknown required versions/semantics refuse mutation.
A moved library retains its ID and can reuse compatible external cache facts after explicit root validation; identical names never prove identity.
A copied library with the same lineage cannot bypass local root selection or exposure approval.

## Pages And Source Pointers

Pages use core Open Knowledge Format 0.2 conventions without governed host requirements.
Page frontmatter owns page annotations; `sources/<source-id>.md` owns asset annotations and its Markdown body is the authored summary.
The source ID comes from the pointer filename, not a content hash.
Pointer frontmatter contains `mount_id`, `path`, `digest_state`, `digest`, `size_bytes`, `media_type`, `file_id`, `availability`, `title`, `description`, and `tags`.
It may preserve optional origin/capture facts and authored extensions when known, but never fabricates them.
A verified `digest` is lowercase SHA-256; an unverified pointer uses `digest_state: unverified` and `digest: null`.
Digest-required operations hash first or return missing coverage; neither size nor a placeholder supplies a digest.
Backend-qualified file ID evidence may be null and is never content version identity.
Raw frontmatter and authored extension fields survive annotation updates; rewrite only the requested recognized fields.
Preserve exact tag spelling; retrieval can compare whole tags case-insensitively without changing the authored values.
Pre-release hidden sidecars are not an annotation authority for version 2.
Cheap discovery does not require a pointer file for every unannotated NAS object.
Durable annotations create/update a pointer; journal/inventory mappings support unannotated discovery without a second authored annotation registry.
Deleted/unavailable assets retain their pointers and annotations as tombstones, not garbage-collectable cache.
User-supplied or corrected timed transcripts are visible authored companion files with their own digest and recording link.
Generated ASR remains derived cache data until the user explicitly adopts/corrects it.

## Mount Backend And Identity

One `MountBackend` boundary exposes `capabilities`, bounded `list(cursor,budget)`, `stat`, `open_range(expected_version,range)`, and optional `watch`/conditional writes.
These are backend operations, not a provider plugin registry.
Return typed `available`, `offline`, `denied`, `placeholder`, `unknown`, `changed`, `unsupported`, or `deleted` observations.
Offline/denied/incomplete enumeration must not become a successful empty directory.
A cursor is backend observation state, not durable source identity or access permission.

| Backend | Required capability and limitation |
| --- | --- |
| Local directory | Confined regular-file list/stat/range reads, stable file IDs when available, safe create/rename primitives, FSEvents on macOS and inotify on Linux. |
| NAS/SMB/NFS | Use an existing user-selected OS mount; bounded list/stat/range reads and qualified stable server IDs where reliable. Watch reliability defaults false and writes default off. No share authentication or OS mounting. |
| Cloud-synced folder | OS-folder reads plus tested provider residency/conflict-copy metadata. Events are hints; placeholders and unknown cloud residency are metadata-only without selected-file read/hydration authorization. No mandatory cloud account/API. |
| Git remote | Explicit system-Git clone/sync into a visible local working copy, then ordinary local content access. The history owner governs network consent and Git state. |
| S3 | Reserved later capability variation only; no S3 implementation, delta API, or storage-plugin registry in this delivery. |

Never follow an undeclared escaping symlink or admit special files as source bytes.
Non-UTF-8 filenames fail explicitly; regular unknown binary formats remain metadata-searchable.
Recognized or explicitly selected child libraries are boundaries; arbitrary unmarked subfolders remain ordinary folders.
A malformed recognized marker still requires repair/explicit resolution rather than silent parent ownership.
Parent operations exclude child-owned paths; recursive search returns distinct labelled libraries.
Captured work binds the opened root and library identity; root replacement/removal must not redirect work to an ancestor.

Content digest identifies a byte version; source ID identifies an asset; file ID is move/rename evidence.
Qualify file IDs with mount/backend and available volume, generation, or birth-time facts to guard against inode reuse.
Within a backend, reliable continuity plus current observations preserves annotations through moves/renames.
Across mounts, preserve source identity only for an explicit transfer or confirmed association.
Equal digests prove equal bytes, not that two duplicate files are the same asset.
If IDs are unstable/unavailable, retain distinct paths and offer association; never infer a rename from names alone.
Stat snapshots retain size, high-resolution modification/change times, file-ID evidence, provider version, and residency facts.
A same-name new file must not revive a deleted asset's annotations without identity evidence.

## Observation Journal And Rebuildable State

Append one visible JSONL stream per writer at `journal/<writer-id>.jsonl`.
Every record contains `writer_id`, `sequence`, `observation_id`, `operation_id` when applicable, `source_id`, old/new digest and path, `availability`, `evidence_kind`, and `observed_at`.
Digest fields are null for unverified observations; a digest timeline is not retention of historical media bytes.
Operation records also bind intent/affected digests and final outcome/commit where applicable.
Use writer-local monotonic sequence numbers; there is no global ordering, causal clock, consensus, or vector-clock protocol.
Per-writer streams avoid concurrent append races in synced folders.
A truncated final record is recoverable, but retain/report malformed bytes and never silently discard observations.
Do not journal source bodies, query text, raw tool arguments, private keys, or credentials.
Journal append traffic is not an automatic text-commit trigger.
SQLite projections of inventory, event cursors, jobs, and lookup are rebuildable from visible files/journal plus scans.
They are not a hidden annotation authority, and cache eviction cannot delete visible journal history.

## Four Tracking Layers

All four layers are required together.

| Layer | Required behavior |
| --- | --- |
| Verify on read | Before delivering hits or bytes, check current mount availability and source stat/version. Rehash changed/uncertain candidates, withhold stale hits, and recheck after reads for detected concurrent drift. Cached vectors do not make changed originals current. |
| Reliable events | Debounce FSEvents/inotify hints, wait for a quiet period and stable before/after observations, then process. Start/resume catches up or scans when continuity is lost. |
| Reconciliation | Stat-first bounded enumeration and known-descendant inspection; hash new/changed/uncertain candidates or explicit verification requests. Local startup/periodic catch-up; NAS defaults nightly/manual with visible schedule and last-complete time. |
| Cost tiers | Eager metadata, prioritized text extraction, lazy/budgeted idle-time or selected media/ASR/embedding work; pinned/project-priority sources first. Show pending, unsupported, failed, and partial work independently of matches. |

FSEvents persists event IDs with volume/root identity only after the corresponding inventory transaction is durable.
Dropped/coalesced/must-scan/root-change events require affected-tree reconciliation; changed volumes invalidate old cursors.
Inotify has no durable replay; startup, overflow, and watch-limit exhaustion require reconciliation even after a previously healthy watch.
Directory mtime is not proof that descendant content is unchanged.
Prune a subtree only with trustworthy event/delta continuity; otherwise inspect known descendant metadata and perform bounded enumeration.
Coarse timestamps, unreliable IDs, and forced verification enter the hash-candidate set.
Offer full hash verification because restored-stat unobserved edits can defeat cheap freshness checks.
Do not claim stat-only detection of a stat-preserved edit.

Only a complete successful relevant enumeration establishes absence and emits deletion tombstones.
An incomplete, denied, offline, or cancelled scan cannot infer deletion.
Reconnect reconciles tombstones with current identity, not just path names.
Growing files and renamed temporary files remain `writing` until quiet/stable; retry a new version instead of indexing half a file.
Cloud conflict copies are separate assets with an explicit conflict relation; do not auto-merge/delete them or transplant annotations.
Never sniff, hash, or open a cloud-only placeholder merely to identify its type or complete a scan.
Use tested public residency APIs where available; unknown residency on cloud mounts remains metadata-only until explicit authorization.
Selected download/available-offline actions are permitted; recursive mass hydration is never the default.

## Bounded Jobs And Cache

Use one small bounded queue with per-mount concurrency, cancellation, priority, and resource reservations.
A job key binds source digest, recipe, and input dependencies; duplicate delivery is idempotent and newer versions supersede obsolete work.
Use finite classified retries/backoff; offline waits for reconnect, invalid models need intervention, and capacity failures must not spin.
Publish only complete derived generations; interruption preserves the previous generation and truthful coverage.
Interactive exact/regex scans use admitted text/representations, not BM25 candidates, and never synchronously extract a whole NAS.

### Effective Resource Accounting

Use one process-wide memory budget, defaulting to 50% of the physical memory available at process start.
Every job reserves its memory before allocating; charge child-process, runtime, model, and reader-pinned memory to the job that caused those costs.
Keep charges alive while their resources remain live, including after worker completion when readers still pin memory.
When current reservations exhaust the budget, queue work with a visible `waiting for memory` state rather than refusing it or spinning.
Refuse a single job whose required memory exceeds the whole effective budget with a typed error.
Reserve generated storage before writing against the cache volume's available free space, retaining a floor of the greater of 2 GiB and 5% of that volume's capacity.
Physical free-space observations do not themselves settle outstanding reservations or establish that a worker has stopped.
Expose effective memory/storage budgets in status and over MCP, and permit configuration overrides.
Retain existing per-format hostile-input ceilings; this policy introduces no whole-corpus cap.
These are required product defaults and lifetimes, not a claim that a cooperative queue counter physically bounds an opaque processor.

### Private Cache Placement And Lifetime

Resolve an app-owned OS cache root outside every home, mount, cloud-sync folder, and external history store.
If the OS cache root is synced/network-hosted, require an explicit local alternative.
Revalidate overlap when mounts/locations change, rather than trusting only initial configuration.
All extracted text, generated transcripts, clips/keyframes, embeddings, thumbnails/previews, indexes, and private processor temporary output stay there.
Use per-library namespaces, restrictive OS permissions, and visible bounded disk/RAM settings and usage.
Sensitive libraries never share cache namespaces with exposed libraries.
Cache keys are `(content_digest, effective_processor_identity)` with hashed fan-out paths and atomic complete-object installation.
Processor identity includes code/version/options/model/package/space/dimensions/preprocessing and input dependencies.
Corrected transcript digests and embedding-relevant annotations invalidate dependent objects.
A caller accesses an object through its current admitted library/source mapping, never a naked cache hash.
Readers/jobs pin objects; garbage collection deletes only unpinned recognized derived objects and updates coverage after eviction.
Unknown files or symlinks in retired generations remain with cleanup warnings; never broaden cleanup to authored data.
An exhausted budget stops dependent work without deleting originals, annotations, history, or pinned data.
The actual previous complete generation survives failed publication; retirement must respect every active reader pin.

## Safe Writes And Recovery

`apply_text_change(expected,candidate,operation_id)` returns saved, conflict, or indeterminate domain outcomes, mapped by service DTOs.
Expected state includes verified digest and stat/file identity, not mtime alone; compare current bytes before changing existing files.
Serialize omni writers for one home; acquire a fresh admitted owner/generation context for each operation.
Create new files exclusively with no-clobber; import copies and never deletes its source.
Initialization preflights every template destination and writes the manifest last; a failed partial unmarked initialization is reported for inspection before retry.
Import a source named `omni-knowledge.json` as `omni-knowledge.asset.json` so an asset cannot replace the manifest.
Preserve child ownership and destination annotation collisions during explicit transfers.
An explicit transfer preserves annotations; failure removes only its own provably unchanged staged data.
Interrupted staged reservations are inspectable recovery conflicts, never permission to guess and overwrite.
A selected-file inspector reads live properties and authored annotations without loading a full index or hashing source content.
Metadata saving does not synchronously build an index; it reports saved values and pending reconciliation separately.

A cooperative lock plus stat plus unconditional rename is not compare-and-swap against unrelated editors.
Use real conditional-write support where available.
On local filesystems, preserve the displaced target through a no-clobber same-filesystem recovery step before exclusive candidate installation.
Validate the displaced version and final outcome; retain both versions if the displaced bytes differ or another writer creates the destination.
Never overwrite a competitor to simulate rollback, and never retire recovery copies while an external writer may still change them.
For insufficient backend primitives, create an inspectable proposal/conflict file and refuse in-place replacement.
Raw originals are read-only by default.
Unresolved recovery files are visible conflict material, not hidden authored cache.
Record intent and affected digests before multi-file effects; do not claim atomic visibility across files, Git refs, cloud upload, or NAS.
After interruption, inspect actual files, journal, and refs; complete the same operation only when provable, otherwise expose recovery.
Do not blindly replay uncertain writes or create duplicate commits.

Confirmed macOS Trash is not permanent deletion.
It rejects manifests, internal state, symlinks, and child-owned sources, preserves annotations, and reports pending reconciliation.
Library detach removes only the declaration, never the attached originals.

## Pre-release Format And Acceptance

Engineer decision, 2026-11-09: version 1 was a pre-release development format with no installed base; legacy migration is dropped.
Libraries created before `7852835` require a new empty library initialized as version 2 and re-import of their originals.
Keep the old library untouched; version-1 inventory/preview remains readable and native Query or mutation refuses with this re-import guidance.
Do not silently rewrite its marker, move annotations, delete sidecars, or install a migration writer.
New imports write canonical source pointers through the existing owner.

Primary acceptance uses public operations with real disposable files and observable outcomes.
Prove offline is not deletion, rename differs from duplicate content, placeholders remain unopened, and external writer races retain both versions.
Prove cache/history never overlap sync roots, inotify overflow/startup scans reconcile, budget/cancelled work does not publish half a generation, and restart preserves operation identity.
Mac FSEvents, actual cloud residency, and NAS continuity require their own platform observations under product qualification.
No storage-format or documentation gate alone proves those runtime effects.
