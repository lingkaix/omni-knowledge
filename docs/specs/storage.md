# Library Storage And Recovery

Status: Implemented v1 specification under the accepted [product requirements](product.md).
This document owns the v1 representation and lifecycle; [the first storage decision](../decisions/001-files-and-derived-indexes.md) records its rationale.

## Authority

`omni-knowledge.json` declares a root, display name, schema version, stable library identity, and optional local embedding and speech settings.
The only supported configuration version is `1`; unknown versions fail explicitly.
Original regular files remain the content authority.
Every child marker entry is an ownership boundary, including malformed files and dangling links; repair the child before opening it.
Captured desktop jobs require the exact opened root and fail if its marker disappears instead of redirecting to an ancestor.
Editable `.omni/metadata/<SHA256(relative-path)>.json` records exact tags and a description for a root-relative path.
The metadata schema contains `version`, `path`, `tags`, and `description`; source hashes do not silently merge file identities.
Front matter and sidecar tags combine without case folding; every required query tag must be present.
A metadata sidecar with a different path or unsupported version stops indexing instead of being ignored.
An external rename changes path identity; external rename tracking is not implemented.
An explicit desktop transfer copies editable tags and description to the new path and retains the old sidecar for recovery.
File and authored-sidecar collisions never overwrite an existing destination.
Transfers stage a destination sidecar before a no-clobber file operation; a failed file operation removes only its own unchanged staged sidecar.
An interrupted transfer can leave a reserved sidecar; a retry reports the collision for inspection rather than guessing recovery.

The concept layer uses the core Open Knowledge Format 0.2 conventions.
Original raw documents can have arbitrary formats and need not be OKF concepts.
Initialization preserves the two requested original source snapshots and their provenance separately from synthesized reference pages.

## Mutations

Initialization checks all template destinations before writes, refuses collisions, and writes the root marker last.
A filesystem failure during initialization can leave unmarked partial template files; inspect them before retrying.
Import copies a regular source to `raw/` through an atomic no-clobber temporary file.
The reserved marker basename is assigned the destination `omni-knowledge.asset.json`.
Import never deletes the source or replaces an existing destination.
A parent cannot import into a `raw/` directory owned by a nested root.
Metadata replacement uses an atomic JSON write in the same directory.
Desktop metadata saving changes only the authored sidecar; the user explicitly refreshes the index to update retrieval.
The selected-file inspector reads live attributes and authored metadata without loading a snapshot or hashing source contents.
An explicit confirmed desktop action can move an owned regular source to macOS Trash.
This action rejects markers, internal state, symlinks, and child-owned sources and preserves authored metadata for recovery.
It does not permanently delete the source or automatically publish a new index; the status check reports the removal until reconciliation.

A process holds an exclusive `.omni/write.lock` for indexing and metadata mutations.
A second writer fails with a retry message instead of writing concurrently.
External agents should finish their file edits before requesting reconciliation.
The indexer cannot promise an atomic snapshot of files modified outside this cooperative writer boundary.

## Index Publication

Each derived generation contains a JSON manifest, a Tantivy index, and a generated-path ownership inventory.
The manifest records its root, file evidence, extraction status, warnings, timed transcripts, vectors, and model identity.
New roots have a stable `library_id`; their manifests use that identity so the whole folder and its index can move together.
Opening and checking a legacy root do not change its configuration.
A legacy root obtains its identity during an explicit index build; until that upgrade its existing index remains bound to its original path.
A distinct initialized root cannot use another root's index merely because their display names match.
Hash changes invalidate content reuse; transcript and ASR identities also invalidate media reuse.
Build a complete generation before atomically replacing `.omni/current.json`.
A provider preflight, response identity, transport, or invalid-vector failure aborts publication and preserves the prior pointer.
Optional local parsing or media preparation failures are reported as missing coverage while filenames and metadata remain searchable.
Lexical reconciliation drops stale vectors for changed sources until a requested embedding build refreshes them.

Readers hold a shared `.omni/read.lock` through one search.
Process-local search caches may retain validated immutable data, but each query must recheck its current owner and generation under a fresh reader lease.
Changed manifest identity or configuration must prevent reuse of incompatible cached state.
An idle cache must not prevent publication or retirement.
Publication takes this gate exclusively for the pointer switch and retirement, after the expensive build completes.
Retain the newly published generation and the actual previous pointer target.
Retire only recognized older generated state inside the selected root, using its complete file and directory inventory.
Older generations without an inventory remain retained with a warning.
Unexpected data or symlinks in old state are preserved with a cleanup warning.
This protocol covers cooperating processes and atomic local filesystem operations; it does not claim resilience to storage loss or arbitrary concurrent filesystem replacement.

## Recovery

Back up originals, authored wiki pages, the root configuration, and `.omni/metadata/`.
Derived indexes and temporary clips can be rebuilt.
Run `omni index` to reconcile additions, modifications, and deletions.
Use `--embeddings` with the compatible local service to rebuild vectors.
A corrupt or foreign manifest is never used for search; an explicit index build can replace invalid derived state while preserving source metadata.
Inspect retained cleanup warnings rather than deleting unknown data automatically.
Nested libraries must be independently indexed; recursive search reports each source root.

## Background Status

The desktop compares owned source paths, sizes, and modification times with a lightweight published-manifest projection.
It checks editable-metadata and configuration timestamps and the presence of lexical index metadata and referenced segment files.
This observation does not verify every source hash or every index byte.
An equal-size edit with its original timestamp restored needs explicit indexing for hash reconciliation.
An incomplete or bounded scan never reports unchanged source metadata as a completed result.
Status checks do not initialize folders, publish indexes, install tools, or start services.
