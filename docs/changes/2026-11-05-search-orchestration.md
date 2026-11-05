# Search orchestration

Date: 2026-11-05.
Status: root lexical path uses the vendored kernel. Not READY. Not a second lexical engine.

## What changed

`knowledge-search` publishes kernel partitions and queries them. Lexical rank lists fuse with `sum(1/(60+rank))`.
Exact and regex results stay in source-then-location order and keep distinct body line offsets.
Path, title, and description are scanned before the body, so a filename hit has no source line. Tags stay on the kernel title for lexical rank and out of the regex scan.
`VectorStore` is a disk generation with eligibility-before-read, identity checks, and pins.
`src/search.rs` publishes that kernel store from `build_bm25` so the existing ownership inventory includes it, then queries it.
`build_trigrams` no longer builds a lexical index. Public search signatures are unchanged.
Case-insensitive grep is a kernel regex with the existing literal escape on the desktop path.

## Account and facade

The shared account is `d51cdde487a707cfcda5050655e7bac73943f7c7`.
This base has no `knowledge_core::admission` module, and Core is not leased, so the root consumer passes a measured pending grant.
That grant is not a reservation and not a second pool. The published call remains `Account::current()?.reserve(Reservation { memory_bytes, temporary_bytes }, Origin::Job { .. })` before `search::search`.
The committed adapter guide leaves partition planning, RRF, and disk vectors to this owner. `KernelAdapter` is not in this base. This lease calls `retrieval_kernel::Kernel`. Root lib cutover stays with its owner.

## Gaps

Root semantic ranking still reads snapshot vectors. No publisher in this lease writes a `VectorStore` segment.
No Mac RSS child is started. Hosted CI and macOS were not run.
The embedding fixture proves loopback transport and declared identity only.
