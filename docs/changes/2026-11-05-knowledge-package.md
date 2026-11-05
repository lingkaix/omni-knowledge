# Knowledge Package Capture And Staging

## Source And Scope

The package owner implemented native ZIP capture, preview, and private restore staging on `40e8e01f000231bfca399b1c8115641ae4b86e93`.
The exclusive paths are `crates/knowledge-package/**` and this record. Root membership and the lock changed only to add that crate and its already locked dependencies.
Storage apply, Git transactions, EPUB, and root callers are not edited here.

## Result

`inspect`, `capture`, `stage`, `apply`, and `v1_import_plan` are the frozen public calls.
Framing rejects hostile names, links, encryption, overlap, truncation, checksum failure, and ZIP64 disagreement before treating an archive as installed.
Payload expansion uses actual output counts and the `Expansion` seam. Declared sizes do not allocate the output buffer.
Capture links a validated private archive only when the destination is absent. Staging keeps an incomplete generation for inspection and does not delete unrelated private files.
`apply` forwards core and original files to storage and an inert history bundle to the history owner.

## Remaining Integration

Link `Expansion` to `knowledge_core::admission::Holding::grow` and `written` when that module is in this workspace.
Storage still performs `apply_text_change` and exclusive original create. History still verifies and imports bundles.
Direct STORED index search, SSH signature trust, and foreign v1 publication remain unwired. EPUB stays with processing.
