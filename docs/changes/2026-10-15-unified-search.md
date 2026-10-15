# Unified Search And File-Specific Results

Date: 2026-10-15.
Source: The engineer requested independently enabled search chips, shared comma-separated keywords, case-insensitive text and tags, and file-specific result presentation.
Owners: [Product](../specs/product.md), [retrieval](../specs/search.md), and [native workspace](../decisions/003-native-preview.md).

## Outcome

The query field supplies every enabled method: Full text, Text, Meaning, File names and notes, and Tags.
A compact chip row replaces the single-mode dropdown and separate search-tag field.
ASCII and Chinese commas separate alternatives; blank entries and repeated lowercase equivalents are ignored.
Any keyword through any enabled method can retrieve a file.
Tags match whole values without case sensitivity and do not restrict other enabled methods.
Text treats punctuation literally; the existing CLI regex mode and required-tag filters remain available.
Default required-tag filters also ignore case, including previously published Tantivy indexes; an explicit CLI case-sensitive request retains its override.

The shared core reads one validated snapshot and fuses multiple lists with one reciprocal-rank vote per file per method and keyword.
Single-method, single-keyword retrieval retains its original order.
Distinct line and time locations survive fusion; filename-only hits no longer invent source line 1.
The desktop keeps one row for an image or an unlocated source, expands text only for distinct lines, and expands recordings only for distinct time ranges.
Counts distinguish files, useful locations, and retained retrieval records rather than claiming every vector is a separate match.
Draft methods remain separate from applied conditions; invalid or failed requests retain the previous successful results.

## Verification

`cargo test --offline --release --features gui` passed 181 tests with zero failures and one existing ignored real-ASR test.
`cargo clippy --offline --release --all-targets --features gui -- -D warnings` passed.
`cargo fmt --all -- --check` and `git diff --check` passed.
The image-row regression failed before implementation, then passed with single image leaves, deduplicated text lines, SVG source navigation, and recording ranges.
Core tests cover normalized alternatives, peer tag retrieval, old-index tag variants, literal punctuation, unchanged single-method order, dense file ranks, coordinates, and explicit method failures.
Independent review found no remaining introduced issues in the final contracts, implementation, or test assertions.

Native build 13 retrieved two fixture files for mixed-case comma-separated text, exposed their distinct line locations, and highlighted Markdown line 8 after selecting its child.
Tags alone retrieved the existing mixed-case front-matter tags through the same field without reindexing that fixture.
Text and Tags could be enabled together; disabling all methods retained the last successful results with an actionable message.
Comma-only input restored directory browsing even with every method disabled.
The actual photo query `帽子的背影` retained its leading order, showed `50 / 321 candidates`, and displayed photos as single rows without expandable vector fragments.
Selecting `DSCF9583.JPG` opened the image directly.

The locally signed version 0.1.0, build 13, is at `dist/Omni Knowledge.app`; strict deep signature verification and plist validation passed.
The app remains on the photo library and query with Meaning enabled and that photo selected.
No existing user source, authored metadata, library configuration, or photo index was changed or refreshed; its publication-pointer hash is unchanged.
No service, dependency, or model was installed or changed.
Native checks used an isolated `/private/tmp` library; test output and observation facts remain under ignored `temp/unified-search/`.
The disposable library was removed after restoring the photo workspace.

## Limits

Meaning requires the existing compatible local provider and indexed vectors; its similarity scores still have no calibrated relevance threshold.
Keywords containing commas cannot represent one literal keyword in this interface; the CLI retains its original single-query behavior.
Queries and evidence remain bounded by the existing 10,000-record result limit.
