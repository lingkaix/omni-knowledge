# Retrieval Semantics

Status: Implemented v1 specification under [product requirements](product.md).
This document owns observable retrieval behavior; [decision 002](../decisions/002-retrieval-and-local-model.md) records the implementation rationale.

## Scope And Evidence

Every query reads the last successfully published generation for one root.
Process-local reuse must recheck root ownership, current configuration, generation, and manifest identity under a fresh reader lease; it cannot return results from an obsolete generation.
A successful reconciliation removes deleted paths and refreshes changed content.
A query does not claim an atomic live-filesystem view.
Result records include `root`, relative `path`, display title, bounded snippet, mode, relevance score, exact tags, and available line or time evidence.
Line numbers refer to original text files, including Markdown front matter, or derived lines for HTML, EPUB, PDF, and Office content.
Speech matches use recording ranges instead of invented lines in a binary recording.
The software returns candidates and evidence; scores are not truth probabilities.
Semantic scoring returns ranked vector candidates without a calibrated relevance threshold, so a returned fragment is not a confirmed content match.
The desktop groups occurrences by file in first-occurrence order without exposing repeated vector fragments as separate image results.
Text children represent distinct line locations; recording children represent distinct time ranges.
An image or another source without navigable locations has one row, while retained evidence counts remain available separately.
It labels semantic and hybrid files as candidates, counts distinct returned files separately from fragments, and does not imply a full-corpus total at the result bound.
It initially displays 50 distinct files and permits further files to be revealed without another query.
Directory order applies to ordinary browsing and deterministic grep output; it cannot override ranked BM25, semantic, or hybrid presentation.

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
Existing CLI and MCP tag filters require every complete selected value before result limiting and ignore case by default.
The CLI's explicit case-sensitive option also applies to these exact tag filters.
Original tag spelling remains authored data; case-insensitive retrieval works with previously published indexes without rebuilding them.
`semantic` scores compatible local vectors from text fragments, media metadata, images, timed video clips, and speech transcripts.
Text and image queries share the declared model space.
A stopped or incompatible provider produces an explicit error and does not silently return an empty semantic corpus.

`hybrid` combines lexical and semantic file ranks using reciprocal-rank fusion with constant 60.
Repeated fragments do not give a file extra votes.
Timed semantic evidence is retained for a fused recording result when available.
Recursive CLI search runs each library independently and returns its root identity.
BM25 scores from different libraries are not calibrated probabilities; cross-root ordering is a useful first-version presentation, not a global shared index.

## Unified Desktop Query

The desktop enables full text, literal text, meaning, file names and notes, and tags independently.
Full text is enabled by default.
One input supplies keywords to every enabled method; ASCII and Chinese commas separate alternatives.
Trim each keyword, discard empty entries, and deduplicate case-insensitively.
Any keyword through any enabled method can retrieve a file; tag keywords match whole values and are not hidden mandatory filters.
Literal text treats punctuation as text rather than a regular expression; the existing CLI grep mode retains regular expressions.
An empty input returns live browsing in the desktop and indexed navigation through the unified core API.
A nonempty input with no selected method returns an actionable error.
Multiple keyword or method lists combine file ranks with reciprocal-rank fusion and constant 60.
Each file contributes once per list; duplicate fragments do not multiply its votes.
Preserve distinct available line and time evidence beneath each fused file without letting it change the file rank.
A single method and keyword retain that method's ordering.
Failure of a selected method fails the request explicitly; the desktop retains its last successful results and their applied settings.
This interface does not replace existing CLI or MCP single-mode queries and required-tag filters.

## Bounds And Coverage

Results are bounded to a requested maximum of 10,000, and each returned snippet is at most 500 Unicode characters.
Extraction has an 8 MiB source/output limit; metadata stays indexed when content exceeds the limit.
Semantic request chunks contain at most 1,600 Unicode characters with a small overlap.
Provider dimensions, model, package, and space must match the generation; incompatible vectors are never mixed.
Missing local parsing or media-preparation coverage is explicit in `status` and the index report.
Exact cosine scoring and JSON snapshots trade simplicity for memory use.
A bounded process-local snapshot cache removes repeated parsing for unchanged generations; the first query after a restart or publication still loads and validates the full manifest.
Local measurements and their library size belong in the [search performance record](../changes/2026-10-15-search-performance.md); they are not latency guarantees or a recall benchmark.
