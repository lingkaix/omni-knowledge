# Search Latency Correction

Source: The engineer reported excessive waiting for results across search modes.
The [retrieval specification](../specs/search.md) owns result semantics, the [storage specification](../specs/storage.md) owns publication safety, and [decision 002](../decisions/002-retrieval-and-local-model.md) records the chosen optimization.

## Result

Version 0.1.0, build 11, buffers large manifest reads and retains validated immutable snapshots for the two most recently queried roots.
Every query opens a fresh reader lease and checks the canonical root, current configuration, publication pointer, confined paths, and manifest file identity before reuse.
The cache contains no retained reader locks and does not change the index format, retrieval algorithms, filters, or limits.
Semantic requests use the embedding boundary's fresh compatibility preflight, removing the redundant outer preflight.
The desktop runs bounded local and model search workers independently and keeps only the newest pending query across both workers.
Obsolete results cannot replace the latest query, and obsolete workers do not keep the current search indicator active.

## Measurements

Measurements ran on Apple M5 Pro with 64 GB memory and macOS 27.0.1 against one unchanged publication containing 321 records and 895 vectors of dimension 2,048.
Its manifest contains 42,054,468 bytes.
Unbuffered parsing issued 42,054,469 file reads in a counted parser probe; a 64 KiB buffer reduced this to 643 reads.
The original repeated in-process Files query still took about 18.4 seconds, which excluded process startup as the main cause.

| Query | Previous CLI observation | New CLI median, three fresh processes |
| --- | --- | --- |
| All files, limit 10,000 | 11,875 ms | 106 ms |
| Exact tag `knowledge`, limit 10,000 | 12,079 ms | 105 ms |
| Grep `knowledge`, limit 30 | 18,540 ms | 106 ms |
| BM25 `knowledge`, limit 30 | 18,766 ms | 105 ms |

Files, tag, and grep measurements used the original root without mutation.
BM25 used an isolated copy of the same derived publication because the restricted runner cannot create Tantivy's metadata lock in the original root.
Each comparison used the same root argument and generation; complete serialized result hashes matched, including order, scores, snippets, and evidence.
The new CLI values include process startup and full manifest validation; the operating-system file cache was not cleared.

A resident harness reopened the knowledge base before every call, matching the desktop's root-opening pattern.
The first Files load took 102 ms; its next call took 0.214 ms.
In the copied publication, subsequent Files, tag, BM25, and grep calls took 0.275, 0.180, 0.826, and 0.888 ms respectively.
These core measurements exclude native rendering and do not establish a general latency guarantee or ranking-quality benchmark.

With the existing local WeMM provider already ready, a fixed `flower` query on the copied publication took 12,600 ms before and 170 ms after for semantic search, and 12,874 ms before and 141 ms after for hybrid search.
Each pair returned identical full-result hashes, including scores and ordered path, line, and time evidence.
Resident semantic and hybrid calls took 21–53 ms after snapshot warmup.
These model observations exclude service startup and use one new-process sample per version and mode.

## Verification

Checks ran from the repository root with Rust/Cargo 1.99.0.

| Command or observation | Result |
| --- | --- |
| `cargo test --offline --release --features gui` | 163 passed; 0 failed; 1 existing local-ASR test ignored |
| `cargo test --offline --release --test search_cache -- --test-threads=1` | 10 passed; fresh-provider cases ran with loopback permission |
| `cargo test --offline --release --test knowledge_base moved_library_preserves_cached_vector_evidence` | 1 passed after the mock socket correction |
| `cargo clippy --offline --release --all-targets --features gui -- -D warnings` | Passed |
| `cargo fmt --all -- --check`, `git diff --check`, and local Markdown links | Passed |
| `sh scripts/bundle-macos.sh` | Build 11 created; deep, strict ad-hoc signature verification passed; previous bundle preserved |
| Fresh LaunchServices launch | Signed app opened the temporary library and displayed its actual native window |
| Slow-model isolation | While the fixture delayed its response for 45 seconds, a newer BM25 query returned its sole expected result and that file opened in preview |
| Obsolete completion | The delayed semantic reply left the latest query, result, and selected preview intact; the current search indicator was already clear |
| Empty query | Enter on an empty query restored directory browsing and removed stale search feedback |
| Restored workspace | App returned to the photo library, previous Chinese semantic query, and selected document preview |

Regressions cover full-vector validation, same-size manifest edits with restored mtime, changed publications, root ownership and relocation, confinement, missing generations, configuration changes, reader leases, bounded reuse, and stopped or incompatible providers.
Independent review found no introduced product correctness issue; its mock-socket reliability finding was corrected and checked.
The initial restricted serial provider test encountered a loopback bind denial; the permission-enabled rerun completed all ten checks.

## Boundaries

The first query after restart, cache eviction, or a changed generation still parses and validates its manifest.
The two-root cache bounds entry count rather than bytes; larger manifests require correspondingly more resident memory.
Semantic and hybrid queries still require the configured local model; a model request already in progress finishes or reaches its existing timeout.
The worker change does not claim transport cancellation or make a second model request run concurrently in the same window.
No original files, authored metadata, or original publication pointer were changed during measurement.
Raw timing probes and isolated fixtures remain under ignored `temp/search-performance/`.
Both temporary model servers stopped after verification; the existing real provider was not restarted or reconfigured.
No dependency installation, remote publication, push, or Applications installation occurred.
