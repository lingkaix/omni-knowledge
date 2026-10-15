# Retrieval Semantics

Status: Implemented v1 specification under [product requirements](product.md).
This document owns observable retrieval behavior; [decision 002](../decisions/002-retrieval-and-local-model.md) records the implementation rationale.

## Scope And Evidence

Every query reads the last successfully published generation for one root.
Process-local reuse must recheck root ownership, current configuration, generation, and manifest identity under a fresh reader lease; it cannot return results from an obsolete generation.
A successful reconciliation removes deleted paths and refreshes changed content.
A query does not claim an atomic live-filesystem view.
Result records include `root`, relative `path`, display title, bounded snippet, mode, relevance score, exact tags, and available line or time evidence.
Line numbers refer to original text files or derived lines for converted PDF and Office content.
Speech matches use recording ranges instead of invented lines in a binary recording.
The software returns candidates and evidence; scores are not truth probabilities.

## Strategies

`files` browses indexed paths, titles, and descriptions, including unknown binary formats.
An empty lexical query uses this navigation behavior.
`grep` verifies Rust regex patterns against extracted lines, then filename and description fallback.
Short patterns, regex alternatives, and insensitive matching cannot be eliminated by an unsafe trigram filter.
Only case-sensitive literal patterns with useful trigrams narrow candidate files before verification.
Results have deterministic path and line ordering.

`bm25` uses indexed title/path and content/metadata fields with a title boost.
English words and Chinese characters/bigrams enter the tokenizer projection.
Query terms use relevance union semantics; use grep for an exact text requirement.
Every selected tag is an exact, case-sensitive required filter applied before result limiting.
`semantic` scores compatible local vectors from text fragments, media metadata, images, timed video clips, and speech transcripts.
Text and image queries share the declared model space.
A stopped or incompatible provider produces an explicit error and does not silently return an empty semantic corpus.

`hybrid` combines lexical and semantic file ranks using reciprocal-rank fusion with constant 60.
Repeated fragments do not give a file extra votes.
Timed semantic evidence is retained for a fused recording result when available.
Recursive CLI search runs each library independently and returns its root identity.
BM25 scores from different libraries are not calibrated probabilities; cross-root ordering is a useful first-version presentation, not a global shared index.

## Bounds And Coverage

Results are bounded to a requested maximum of 10,000, and each returned snippet is at most 500 Unicode characters.
Extraction has an 8 MiB source/output limit; metadata stays indexed when content exceeds the limit.
Semantic request chunks contain at most 1,600 Unicode characters with a small overlap.
Provider dimensions, model, package, and space must match the generation; incompatible vectors are never mixed.
Missing local parsing or media-preparation coverage is explicit in `status` and the index report.
Exact cosine scoring and JSON snapshots trade simplicity for memory use.
A bounded process-local snapshot cache removes repeated parsing for unchanged generations; the first query after a restart or publication still loads and validates the full manifest.
Local measurements and their library size belong in the [search performance record](../changes/2026-10-15-search-performance.md); they are not latency guarantees or a recall benchmark.
