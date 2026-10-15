# Files And Derived Indexes

Date: 2026-10-14.
Status: Implementation decision under the engineer's explicit instruction to explore and design; not a new restriction on product intent.
Source: The engineer requested arbitrary file management, general metadata, nested knowledge libraries, LLM Wiki, and Open Knowledge Format.
Owner: [Product requirements](../specs/product.md).

## Decision

Keep original regular files and authored metadata as the durable authority.
Use a JSON root configuration as both the marker and the local settings file.
Store editable metadata separately from generated index generations.
Keep generated indexes inside the root and use a stable UUID to preserve identity when the whole library moves.
Assign a missing UUID only during explicit indexing; reading a legacy root does not mutate it.
The nearest root owns a file; parent indexing stops at child roots.
Allow recursive search to combine independently indexed libraries with explicit root identity.
Copy imports without deleting their original sources or replacing same-name destinations.
Publish a complete lexical and vector snapshot through a single atomic current-generation pointer.
Use a writer lock for indexing and metadata mutations.
Readers hold a shared lease; take its gate exclusively only for publication and retirement.
Retain the new current and actual prior pointer target.
Retire only inventoried generated paths; preserve unknown nested content and report a warning.
Use no-clobber desktop copy and move operations, copy authored metadata to the new path, and retain the old sidecar for recovery.
Use confirmed macOS Trash for source removal and keep authored metadata until the user reconciles or recovers the source.

## Alternatives And Consequences

A database-only library could simplify schema queries but would make agent edits and recovery less transparent.
Sidecars next to every source would be easy to discover but could pollute imported folders and create ambiguous filenames.
Central path-keyed JSON metadata keeps arbitrary binary sources searchable without altering them.
External file moves create a new path identity; explicit app transfers can preserve annotations because the user supplied the source and destination.
Content hashes provide evidence but do not imply user intent to merge annotations.
The old absolute-root identity prevented index reuse after a whole-library move; UUID ownership keeps that move distinct from substituting a foreign index.
Index snapshots are explicit; successful indexing reconciles modifications and removals.
A failed build leaves the previous published snapshot readable.
File names and descriptions remain searchable when a parser cannot read content.

## Revisit Conditions

Revisit metadata identity when rename tracking is needed.
Revisit manifest storage and vector indexing after representative personal-library measurements justify a database or approximate index.
Retain source authority and no-clobber import semantics across any replacement.
