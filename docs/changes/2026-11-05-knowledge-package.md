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
The residual repair starts from exact `218e70b953306a3eaa56235b2ffba94de8adaaba`.
Capture now streams actual source reads to a native private `ZipWriter`, hashes each source window, and writes the final canonical manifest without retaining payload bodies or rendering an archive in RAM.
A gated native `Write + Seek` reserves prospective new extent before each write, including headers, central-directory/ZIP64 records, and compression expansion, then reconciles actual short writes.
Metadata admission now preflights JSON structure and string bytes before serde/JCS/semantic graph allocation and funds capture clones before construction.
RFC 8785 and the pinned canonicalizer are unchanged.
The previous unowned 1 MiB parse cutoff is removed; actual manifest reading, graph admission, and processing remain bounded by the explicit job quotas and shared account.
A complete generation is reused only for the same snapshot, roles, digests, and on-disk bytes. Conflicts are recomputed from the current destination. A truncated or different archive does not adopt the old plan. Tampered staged bytes make `apply` refuse before `SafeApply`.
Recovery now uses held no-follow directory descriptors for every relative component, a fresh canonical `MANIFEST` matched to the sealed snapshot, and incremental exact `PLAN` comparison including the operation and EOF.
Apply delegates a native `VerifiedSource` with the same `Work`, not a mutable path; destination owners must verify the exact copied stream in private output before publication.
The native descriptor version is checked before and after hashing/copying.
Unknown and previous-generation evidence is not deleted or automatically migrated.
Manifest bytes are RFC 8785 via `serde_json_canonicalizer` 0.4.1. Optional numbers and UTF-16 key order are accepted only in that form.
A compatible index, including complete coverage, stays installable with fallback `kernel_not_wired`. `required_features` still refuse.

## Residual Repair Evidence And Consequential Review

All 12 retained public cases remain, with only the native-source/`Work` adapter updates required by the breaking API.
Six added bounded owned-fixture cases cover a real 4 MiB source through capture/inspect/stage/apply under a 3 MiB memory ceiling, optional small-value graph admission with a roomy positive control, archive-extent refusal, bounded recovery, a normal no-follow ancestor refusal, and cooperative cancellation/deadline controls.
No network security tests, exploitation demonstrations, fake I/O, or payload-spy performance claims were used.
The compiled owner guide is included in the crate documentation test.
Raw setup, red API compilation, and subsequent gate output are retained in the invocation-owned `m5-residual-repair/evidence` directory.
Package check/all-targets, strict Clippy/all-targets, 18 public tests, one compiled guide, strict documentation, and workspace format gates pass with locked/offline Cargo, two build jobs, private target/TMPDIR, and a hash-verified private dictionary supply.
The broader `cargo test -p knowledge-core --locked --offline` attempt fails in 16 cache cases because its nested consumer lockfile requires updating before the consumer can compile; Core and its lockfile are outside this writer scope and are not modified.
That broader gate is not reported as green; no root or cross-module acceptance is inferred from package success.
The directly related Core checked-value, job-contract, and manifest-codec consumers pass separately: 7 + 17 + 19 cases, with no edits or skipped tests.

The affected consequential self-review checked the actual capture gate, metadata lifetimes, recovery binding, and owner handoff after the genuine fix.
It found and corrected short-write reconciliation in staged writes, repeated name lookups, special-node prechecks, signature text workspace admission, and new-`Work` accounting of already retained holding memory.
This is source-level review with public API runtime regression evidence, not an independent review or a runtime leak claim.
No independent agent was started because the task excluded agents; external consequential acceptance remains with the lead.
Linux native gates do not certify macOS/Windows effects or the unfinished cross-owner integrations below.

## Still required

`crates/knowledge-package/OWNER.md` lists the unfinished calls. It does not close them.
SSH signer policy, kernel search of STORED generations and float32 vectors, genuine v1 mapping and export context, `git bundle verify` on history `2c86db6`, M1 `Store::install` once storage is a committed member, and EPUB through processing `9a807f9` remain required.
Do not add a second Git engine, destination writer, or account. Admission stays P1 `d51cdde`.
