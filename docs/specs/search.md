# Retrieval Semantics

Status: Accepted full-version contract, 2026-11-04; not an implementation or benchmark report.
This document owns lexical/metadata/vector retrieval, file-level fusion, generation identity, partitioning, evidence, and coverage.
[Service](service.md) owns wire DTOs; [storage](storage.md) owns source freshness and exclusion-safe admitted access.
[Processing](processing.md) owns extraction, representations, model validation, and original coordinates.
[Packages](package.md) owns the archive container; this owner defines its searchable generation seam.
This supersedes legacy snapshot-only results, competing omni tokenization, and resident JSON-vector storage.

## One Reused Lexical Kernel

Use the public `retrieval-kernel` crate pinned to upstream revision `b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1`.
The starting public dependency is `retrieval-kernel = { git = "https://github.com/lingkaix/openkit", rev = "b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1" }`.
M0 must prove an actual downstream build/resolution; source availability or a Cargo declaration alone is not proof.
For the full build, vendor the exact pinned crate once because its filesystem-only private store/query machinery lacks the required mount/archive seam.
Retain upstream notices, dictionary supply/licenses, revision, upstream root-lock evidence, actual consuming workspace lock, and a narrow patch ledger.
Mark every seam patch as a future upstream change candidate; make no OpenKit source edit in this delivery.
Do not permanently build both a Git and a vendored kernel engine, or retain omni's old `search.rs` as competing production lexical authority.

Required generic upstreamable seams are admitted byte sources, read-only Directory/generation providers, caller budgets, and bounded inventory streaming/shard planning.
They must not import omni library types, service exposure, GUI state, or OpenKit governance.
An internal opaque partition key is an adapter detail, not an owner scope in the product data model.
Keep analyzers, scoring, matching, and coverage semantics unchanged except where a separately owned requirement justifies an amendment.
Use Tantivy 0.26.2 with Jieba and packaged Lindera/IPADIC identity; preserve per-document language selection and English/Chinese/Japanese golden cases.
The old omni character/bigram behavior needs independent characterization; intentional migration to accepted kernel semantics is a behavior-change ratchet, not a reason to delete still-required recall checks.

The unmodified kernel's fixed 10,000-document, 8 MiB inventory-frame, and 512 MiB temporary-generation limits cannot be solved by claiming unbounded large-library support.
Use caller-admitted budgets, streaming inventory, and deterministic bounded partitions, not an indiscriminate cap increase.
A loose-file downstream build does not prove archive/multi-mount functionality.

## Query Semantics

One request independently selects BM25, literal, regex, filename/description, exact tags, and semantic methods.
Browsing is listing, not an invalid empty search; live browsing includes unindexed files and empty directories.
Unsupported binaries remain discoverable by path/title/description/tags and typed extraction status.
Queries do not synchronously trigger whole-NAS hashing, extraction, or embeddings.
Separate lexical modes remain available with no model installed/running.
A requested semantic method with an unavailable/incompatible provider fails explicitly; hybrid cannot silently turn into lexical-only success.
Budget/cancellation-limited scans may return partial results with coverage, but a selected-method dependency failure fails the request.

BM25 uses the reused kernel analyzer/scoring behavior with declared title/path/content/metadata projection and title relevance.
BM25 union relevance is not byte-exact matching and scores are not truth probabilities.
Literal matching preserves kernel byte/case semantics.
The desktop's case-insensitive literal method uses escaped line regex and is labelled literal/case-insensitive, not byte-exact.
Regex preserves the kernel LF-line dialect, line boundaries, occurrence locations, and deterministic source/location ordering.
Trigram candidates are conservative accelerators only; short/ambiguous patterns, alternatives, and case transformations fall back to canonical scans where necessary.
Exact/regex scans read every admitted partition/representation, not just BM25 top candidates.
A semantic candidate is neither proof of a literal match nor proof of absence in an unprocessed binary body.

Required-tag filters match every complete value before result limiting, case-insensitively by default.
The explicit case-sensitive option applies to tag comparison as well as text semantics where supported.
Original tag spelling remains authored data; retrieval normalization does not rewrite it or require a rebuild for a case option.
The desktop's tags method instead treats comma-separated keywords as alternative peer retrieval lists, not hidden mandatory filters.
ASCII/Chinese commas split input; trim, remove empties, and deduplicate case-insensitively.
Any keyword through any selected method can retrieve a file; nonempty input with no selected method fails actionably.
Preserve existing CLI/MCP single-mode aliases and all-required tag intent while mapping them to the shared request.
Legacy grep remains regex; desktop literal punctuation stays literal.

## File Fusion And Display Evidence

RRF is `sum(1/(60+rank))`, with 1-based distinct file ranks per method/keyword list.
A file contributes at most once per list, irrespective of fragment/clip count.
A single keyword and method retain that method's ordering.
Retain distinct text/page/time evidence under the file without multiplying its fusion vote.
Exact/regex occurrence enumeration uses deterministic source/location ordering; ranked methods keep their ranked file order.
For tied fused scores, use stable source logical path and source ID order.
Do not compare raw BM25 scores across independent partitions/libraries; fuse declared rank lists with file deduplication.
Independent libraries remain labelled groups rather than pretending they share one statistical corpus.

Results carry source/library identity, path/title/tags, bounded snippets, method evidence, model/processor identities, and available original/representation locators.
Direct Markdown lines include frontmatter; extracted lines never masquerade as original binary byte offsets.
Media ranges refer to the original recording, not reset clips.
Scores are finite and semantic candidates have no calibrated match threshold.
Distinct returned-file counts are separate from retained fragments and useful navigable locations.
An image is one file row; text expands for distinct lines and recordings for distinct time ranges.
Unlocated duplicate metadata is not another navigable occurrence.
The desktop initially reveals 50 files and can reveal more from the same result without requerying or directory regrouping.
A failed query leaves prior results and their applied conditions intact.
Clicked line evidence reveals numbered/highlighted text separately from formatted reading.
If a requested location is no longer present, report stale evidence instead of highlighting a substitute.

## Freshness, Eligibility, And Completeness

Every query pins an immutable complete generation tuple and current admitted library/source mappings.
Process-local caches recheck library ownership, configuration/exposure, selected generation, and manifest identity under a fresh reader lease.
Idle cached state cannot prevent publication/retirement, and cannot return another library's or an obsolete generation's facts.
Before delivering hits/bytes, apply storage verify-on-read and service exposure/exclusions, including during long-running queries.
Rehash changed/uncertain candidates and withhold stale hits until reconciled; recheck after reads for detectable concurrent drift.
Do not promise an atomic view of arbitrary external filesystem writers or detection of stat-preserved unobserved edits without full verification.

When source hashing is pending, cheap path/tag/title discovery may use the rebuildable inventory.
Label it metadata observation with an unverified digest, not a fabricated kernel original digest or verified body match.
Unavailable originals, stale observations, pending hashes/extraction/embeddings, unsupported processors, failures, and stopped work remain explicit.
A successful complete scan can remove deleted sources from current retrieval while retaining annotations/journal tombstones.
Offline/incomplete scans cannot manufacture deletion or silently certify current original content from cache.
Exclusions apply before corpus statistics, candidate enumeration, and vector distance eligibility, not only final result filtering.
No names, counts, snippets, or history of excluded sources leak through a public result.

Distinguish bounded top-k from complete occurrence enumeration and distinguish both from corpus processing coverage.
Scan all admitted partitions for an exact/regex completeness claim; report partial if any budget/partition/source is incomplete.
Partitioning identity belongs in the generation/result so rebuilding/sharding does not silently change rank interpretation.
A multi-TB mount never implies all clips are embedded or a 200 ms whole-corpus exact scan.
Service results cap distinct files at 10,000 and snippets at 500 Unicode characters; larger corpora use pagination/selection or truthful top-k coverage.
The old result-record bound must not be confused with a complete corpus count or a maximum inventory size.

## Disk-Backed Vectors And Generations

`VectorStore` exposes immutable-generation build, open, query, pin, and retire operations.
Rows bind source ID/digest, representation/processor identity, locator, modality, and complete model-space identity.
The baseline is exact cosine over disk-backed little-endian IEEE float32 segments with bounded mapping/scanning windows.
Never load a full JSON-vector corpus into RAM; metadata/control manifests do not embed corpus vectors.
Segment manifests bind dimensions, row order/counts, segment hashes, model/package/space/preprocessing, and source/dependency identities.
Reject nonfinite, zero-norm, wrong-length, missing-identity, or incompatible-space vectors even when dimensions match.
Use exact source/tag eligibility before distance work, not post-limit filtering.

One generation binds lexical index, vectors, row metadata, source/representation digests, processors, tokenizer/dictionary supply, budgets/partitioning identity, and model identity.
Build all required tuple members completely before atomically switching the current pointer.
Do not read independent latest lexical/vector state.
Readers/jobs pin the tuple and objects; retirement cannot delete pinned generations.
A failed preflight, provider response, invalid vector, interrupted job, or incomplete tuple preserves the previous complete pointer.
Optional extraction/media failures may publish explicit partial coverage with searchable metadata, never fabricated content coverage.
Lexical reconciliation invalidates changed-source vectors until the requested compatible embedding work completes.
Unknown retained files or generations without a trustworthy ownership inventory remain with warnings rather than broad recursive deletion.
Cache eviction changes coverage and can require rebuilding; it never deletes originals or annotation authority.

## Searchable Frozen Generations

A read-only generation/Directory provider must open compatible STORED Tantivy components directly from held archive ranges.
An unmodified filesystem-only kernel cannot search a ZIP just because its entries are STORED.
Exact float32 segments are also STORED and directly range-readable without whole-archive extraction.
Deflated text is streamed on demand under bounds, optionally into external cache; it is not random mmap-searchable.
Hold the archive/generation identity for the query and reject/reopen after container replacement.
Verify every used index entry digest before first query, caching verification only for unchanged container identity.
Mmap aligns down to OS granularity and slices bounded entry bytes; ZIP-extra padding is an optimization, not correctness authority.
No Lance zero-extraction archive claim is permitted without an actually proven version-specific archive IO adapter.
An incompatible optional index is omitted/rebuilt externally with a reason, not misread or extracted into the authored home.
The required actual writer-to-archive-reader-to-kernel composition must search STORED indexes with extraction disabled.

## Time-Boxed LanceDB Experiment

The experiment is vectors beside Tantivy/canonical scanning, behind the same VectorStore boundary, inside M4.
It never blocks full-version delivery and never changes authored package authority; retain exact segments if it loses.
Start with released pinned LanceDB 0.41.0/Lance 14.0.2, locked minimal downstream features and declared licenses/build dependencies.
Measure exact storage before ANN; no custom HNSW, DB server, embedding SDK replacing the provider boundary, or backend registry.
Native Lance FTS has BM25/CJK/regex capabilities, but replacing the lexical kernel is not this delivery's requirement.
Lance row-RRF, column analyzers, and table MVCC do not alone establish file fusion, per-document language, canonical locators, or whole-tuple publication.

Predeclare target hardware and binary/build/disk/cache/RSS budgets before the run.
Compare identical precomputed vectors at 10k/100k/1M fragments with English/Chinese/Japanese/code and modality slices, exact and sparse filters, cold/warm latency, RSS/disk/build costs, recall, failure, pins, and retirement.
Keep embedding time separate from retrieval time and report unique content counts.
Exact numerical/evidence parity and no eligibility/publication/retirement regression are mandatory.
ANN candidate gates are recall@20 at least 0.98 overall and 0.95 per declared slice, no required CJK golden loss, and no greater than 2% relative per-language nDCG@10 regression.
A performance win needs at least 2x vector p95 improvement at comparable RSS or at least 50% RSS reduction at comparable p95.
A proposed 200 ms warm p95 excludes embedding and is an experiment target, not an accepted corpus-independent SLA.
Configure bounded DB caches explicitly; vendor defaults and payload arithmetic are not measured RSS evidence.
Do not acknowledge controlled-state removal while old recognized generations, versions, previews, or reader caches still retain the item.
Ordinary non-retrievability is not forensic SSD erasure or deletion of original/history/backup bytes.

## Acceptance

Independent public-seam regressions preserve CJK/Japanese recall, canonical byte/case/line regex behavior, short-pattern fallback, tag-before-limit, and file-vote invariants.
Compose real processing output with retrieval to prove coordinates/dependency invalidation and real generations to prove no mixed tuple on faults.
Exercise changed/offline sources, exposure changes during delivery, all-partition completeness, reader pins, budget/cancellation, and generation retirement.
Observe bounded disk-vector reads rather than infer memory behavior from type names.
Characterize required old behavior before deleting the old lexical engine; independent replacement checks pass OLD and NEW for still-required behavior.
Intentional tokenizer/layout/freshness changes instead use owner-derived fail-OLD/pass-NEW checks.
Deterministic Linux providers prove protocol/numeric/failure handling, never WeMM semantic quality.
