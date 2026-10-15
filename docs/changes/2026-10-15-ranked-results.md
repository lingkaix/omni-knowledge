# Ranked Results And Text Locations

Date: 2026-10-15.
Source: The engineer reported that a Chinese photo query put licenses first, counted duplicate matches, and did not reveal text locations.
Owners: [Product](../specs/product.md), [retrieval](../specs/search.md), and [native preview](../decisions/003-native-preview.md).

## Outcome

Active search preserves the core file order instead of reconstructing alphabetic directory groups.
One file row opens its best returned evidence, and expandable children retain the remaining occurrences.
The first page contains 50 distinct files; further pages preserve ranks and do not issue another provider request.
Semantic and hybrid counts say candidate files, with separate evidence counts in the count hint.
Clearing search restores live directory browsing and removes search-specific count help.
No score threshold, source-type boost, model setting, index format, CLI result, or ranking algorithm changed.

Text evidence opens a numbered reading view at its exact line and highlights the row.
Markdown numbering includes original front matter; HTML, EPUB, PDF, and Office coordinates use their indexed extracted text.
Formatted reading remains available for Markdown, HTML, and EPUB through the text-view disclosure.
An SVG text-line hit opens text; ordinary SVG browsing retains image preview.
The text extraction bound matches indexing at 8 MiB, while media resource requests keep their separate 32 MiB bound.
At most 20,000 text rows are rendered around the selected evidence, with an explicit notice for omitted rows.
Unavailable lines produce a stale-evidence notice without highlighting a different row.
Native observation caught a gutter width rounding issue; numbers now use sufficient width and cannot wrap onto multiple lines.

## Direct Observations

The read-only query `戴着帽子微笑` returned 895 vector candidates for 321 distinct indexed files.
The original core order began with `DSCF9587.JPG`, `DSCF9584.JPG`, and `DSCF9588.JPG`; its first 30 occurrences were JPG files.
The first license occurrence ranked 484, demonstrating that directory reconstruction caused its prominent display.
The indexed files consisted of 298 JPG files, eight RAF files, and 15 other files, including initialization articles, licenses, navigation, and HTML reports.
A JPG had separate metadata and visual vectors, so vector counts were not file counts or confirmed matches.
The new presentation preserves this model order without claiming that all returned candidates are relevant.

## Verification

`cargo test --offline --release --features gui` passed 168 tests with zero failures; the existing local-ASR test remained ignored because it requires its pinned model fixture.
`cargo clippy --offline --release --all-targets --features gui -- -D warnings` passed.
After the gutter-only adjustment, `cargo test --offline --release --test extraction` passed all 11 tests and the final native window verified the layout.
`cargo fmt --all -- --check` and `git diff --check` passed.
The relevance adapter regression failed on the original directory grouping, then passed with grouped ranked rows, preserved evidence, and whole-file pagination.
Extraction regressions cover CRLF, blank and Unicode lines, original Markdown coordinates, derived HTML coordinates, untrusted target attributes, absent lines, and bounded distant-line windows.
Independent review checked the changed contracts, grouping, count state, exact occurrence dispatch, source confinement, and renderer flow.

The final native window centered and highlighted plain-text line 321 and Markdown source line 425 with complete, unwrapped gutter numbers.
Selecting a second text occurrence moved the preview from line 321 to line 650.
Closing the Markdown source disclosure restored formatted reading and a visible Mermaid diagram.
The actual photo query showed `DSCF9587.JPG`, `DSCF9584.JPG`, and `DSCF9588.JPG` first, with `50 / 321 candidates`.
**Show 50 more files** changed the count and status to 100 without changing the leading order; the first photo opened successfully.
The final window remains on the photo library, original query, Meaning mode, and first-ranked photo.
The native text fixture ran from `/private/tmp`; the Documents-folder attempt waited for OS file-access authorization, and no system permission was changed.

The locally signed build is version 0.1.0, build 12, at `dist/Omni Knowledge.app`.
The bundle script passed strict deep signature verification and plist validation and preserved the previous artifact.
No package or model was installed, no service was changed, and no existing user source, authored photo metadata, or photo index was changed or refreshed.
The query output and native observation record remain in ignored `temp/ranking-audit/`; the record summarizes actual AX and screenshot observations.
The disposable `/private/tmp` fixture copy was removed after restoring the photo library.

## Limits

Semantic candidates still use the existing model and exact cosine scores without a calibrated relevance cutoff.
The GUI groups the returned bounded evidence set; reaching 10,000 records can omit other corpus files and is explicitly reported.
Text evidence follows the published snapshot, while preview reads the live file; external changes can make an indexed line stale.
A pre-existing BM25 filename-only fallback can report line 1 for a metadata-only binary; the numbered preview refuses an unavailable line.
The search module owns correcting that fallback when the next search-evidence change is undertaken.
