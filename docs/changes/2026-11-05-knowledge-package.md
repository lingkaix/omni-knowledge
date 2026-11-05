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

## Admission

`62228e5` is unchanged. `b4427e9` fetches frozen `d51cdde` without editing that commit's core text.
`Work` now borrows `Holding`. Retained names and kept payloads call `grow` before allocation.
Scratch buffers reserve another holding on `Account::current` and `finish` only after those buffers drop.
`written` runs after a generated payload is on the private volume. This crate does not finish the caller's holding.

## Owner boundaries

`crates/knowledge-package/OWNER.md` is the public handoff.
Storage `apply_text_change` is still the host `SafeApply` call. `knowledge-storage` is not a committed member of this workspace, so this crate does not link it and does not write destinations.
History `2c86db6` is the only Git seam to consume. It has no bundle verifier. `admit_inert_bundle` stays inert. This crate does not run Git.
STORED index query belongs to the retrieval kernel. `compatible: true` stays `Unsupported`.
EPUB stays `knowledge_processing` `read_epub` / `export_epub`. A package snapshot has no EPUB function.
SSH trust and foreign v1 publication stay refused rather than marked valid.
