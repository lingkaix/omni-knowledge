# Retrieval And Local Model Compatibility

Date: 2026-10-14.
Status: Implementation decision within the engineer's selected Rust and local Apple Silicon scope.
Owner: [Product requirements](../specs/product.md).

## Decision

Use Tantivy for indexed BM25 and a shared Rust API for the CLI and GUI.
Read large JSON manifests through a bounded buffer and reuse validated immutable snapshots in a small process-local cache.
Recheck the exact canonical root, current configuration, published generation, confined paths, and manifest file identity before reuse.
Acquire a fresh shared reader lease for each query; idle cached snapshots hold no publication lock.
Keep cache synchronization outside disk parsing, retrieval, and model requests.
Use conservative trigram candidate filtering for eligible literal grep patterns, followed by parallel regex verification.
Keep exact tags separate from relevance scoring.
Provide independent lexical and semantic modes and reciprocal-rank fusion for hybrid results.
Start with exact cosine scoring over local vectors so results are inspectable without an approximate-index tuning stage.
Connect to the existing local WeMM service instead of copying its patched native runtime.
Validate the model identifier, dimension, embedding space, and package fingerprint before sending content or comparing vectors.
Use the embedding request's existing fresh preflight instead of repeating it in the search wrapper.
Store model identity with the derived generation and refuse incompatible semantic queries.
Segment video into timed clips with ffmpeg and retain time ranges in search results.
Audio requires a transcript or an explicitly configured local ASR tool because this WeMM provider does not support audio input.

## Evidence And Alternatives

[Tantivy](https://github.com/quickwit-oss/tantivy) provides Rust BM25 and configurable tokenizers.
[tgrep](https://github.com/microsoft/tgrep) demonstrates conservative trigram candidates followed by regex verification.
[zvec-grep](https://github.com/zvec-ai/zvec-grep) combines lexical and vector retrieval behind one local interface.
[fff](https://github.com/dmtrKovalenko/fff) demonstrates the value of resident file and content indexes.
We use their published design ideas rather than copying their implementations.
The 2026-10-15 latency investigation found that unbuffered JSON parsing issued one read per byte of a 42 MB personal index manifest.
Buffering removes that shared cost from all modes; generation-bound reuse avoids repeated parsing and vector validation in the desktop and MCP process.
Separate bounded desktop lanes let a newer local query run while an obsolete model request finishes.
One global request revision still prevents obsolete results or errors from replacing the current search.
The engineer's local WeMM report establishes an existing service and a measured GPU baseline.
Approximate vector search, service-based grep, and daemon watchers add lifecycle costs that a first usable version does not yet justify.
Lexical modes remain available when the embedding service is stopped.

## Revisit Conditions

Measure representative library size, warm search latency, index time, and ranking quality before replacing exact vector scoring.
Revalidate the provider on macOS or model-package updates.
Do not mix vectors from different packages merely because their dimensions match.
