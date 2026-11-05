# Knowledge Package Capture And Staging

## Source And Scope

The package owner implemented native ZIP capture, preview, and private restore staging on `40e8e01f000231bfca399b1c8115641ae4b86e93`.
The exclusive paths are `crates/knowledge-package/**` and this record. Root membership and the lock changed only to add that crate and its already locked dependencies.
Storage apply, Git transactions, EPUB, and root callers are not edited here.

## Result

`inspect`, `capture`, `stage`, `apply`, and `v1_import_plan` are the frozen public calls.
Framing rejects hostile names, links, encryption, overlap, truncation, checksum failure, and ZIP64 disagreement before treating an archive as installed.
Payload expansion uses actual output counts on the shared holding. Declared sizes do not allocate the output buffer.
Capture links a validated private archive only when the destination is absent. Staging keeps an incomplete generation for inspection and does not delete unrelated private files.
`apply` forwards core and original files to storage and an inert history bundle to the history owner.

## Admission

`62228e5` is unchanged. `b4427e9` fetches frozen `d51cdde` without editing that commit's core text.
`Work` now borrows `Holding`. Retained names and kept payloads call `grow` before allocation.
Scratch buffers reserve another holding on `Account::current` and `finish` only after those buffers drop.
`written` runs after a generated payload is on the private volume. This crate does not finish the caller's holding.

## Framing, bounds, reuse, and canonical JSON

Central and local flags must match. A ZIP64 extra disk other than 0 is hostile.
The ZIP64 end record must end where the locator begins; a tail whose locator names an offset outside that span is hostile and is not sliced.
Call memory is one running total. A 200000-byte manifest does not fit a 90000-byte call.
Capture reserves the rendered archive length before the private write.
A complete generation is reused only for the same snapshot, roles, digests, and on-disk bytes. Conflicts are recomputed from the current destination. A truncated or different archive does not adopt the old plan. Tampered staged bytes make `apply` refuse before `SafeApply`.
Manifest bytes are RFC 8785 via `serde_json_canonicalizer` 0.4.1. Optional numbers and UTF-16 key order are accepted only in that form.
A compatible index, including complete coverage, stays installable with fallback `kernel_not_wired`. `required_features` still refuse.

## Still required

`crates/knowledge-package/OWNER.md` lists the unfinished calls. It does not close them.
SSH signer policy, kernel search of STORED generations and float32 vectors, genuine v1 mapping and export context, `git bundle verify` on history `2c86db6`, M1 `Store::install` once storage is a committed member, and EPUB through processing `9a807f9` remain required.
Do not add a second Git engine, destination writer, or account. Admission stays P1 `d51cdde`.
