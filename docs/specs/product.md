# Product Requirements

Status: Accepted user requirements.
Acceptance source: The engineer's request in the Codex conversation on 2026-10-14, with the desktop and ranked-result follow-ups on 2026-10-15.
This document owns the requested product outcome; implementation defaults below remain replaceable methods under the engineer's instruction to explore and design.

## Outcome

Build a personal knowledge library for Apple Silicon Macs with Rust, a GPUI desktop interface, and an agent-friendly CLI.
Initialize folders with an LLM Wiki workflow and Open Knowledge Format concept documents.
Include the two requested source articles as the first knowledge content, with original offline snapshots and linked reference concepts.
Support nested knowledge libraries and identify each root with a configuration file.
Admit every regular file type, including binary files, into a general metadata search mechanism.
Provide exact tags, fast grep, indexed BM25, and multimodal retrieval.
Provide rendered Markdown and HTML, front matter, Mermaid, syntax highlighting, and EPUB viewing and export.
Support PDF and Office preview and extraction when mature local solutions exist.
Use local WeMM experiments for images and ffmpeg video segments; provide an audio transcription route.
OCR is optional and may be deferred.

## Project License

The engineer selected Apache License 2.0 for this project's original code in the same conversation on 2026-10-14.
The root `LICENSE` and Cargo SPDX declaration record that choice.
Third-party code, model packages, and source articles retain their original licenses and provenance.
This selection does not relicense bundled dependencies or authorize publication.

## Desktop Identity And Setup

The engineer requested a soft-green version of the existing OpenKit mark for the app logo and icon in the desktop-setup follow-up.
Preserve the supplied vector geometry and keep the original source unchanged.
The desktop must include a detailed guide for its features, reading workflow, retrieval modes, and agent interfaces.
The same page shows runtime dependency availability and installation actions.
Use Homebrew installation actions for external tools such as FFmpeg and whisper.cpp instead of including their binaries in the app.
Run an allowlisted installation only after the user selects its clearly labelled button.
Show installation progress, success, and errors without blocking the desktop interface.
The install button opens a fixed Homebrew command in a visible Terminal window, which owns progress and interaction independently of the app.
Report command handoff separately from installation success, and let the user refresh tool availability after Terminal completes.
If Homebrew is absent, show its prerequisite instructions and official installation page.
Speech models and the WeMM model/provider remain explicit local setup; tool installation must not silently download models or change library configuration.
Tool discovery must work after a Finder launch without relying on the interactive shell's PATH.

## Desktop Workspace

The engineer requested a compact workspace in the 2026-10-15 desktop follow-up.
Remove the two brand text lines from the header and use the green mark to return to the main workspace.
Keep one primary search row at the top and preserve room for files and previews.
Put a clear-search action inside the search input; it clears both the query and tag restrictions and restores the live directory tree.
Group search mode and exact tags in a search-options disclosure instead of a permanent second search toolbar.
Keep active restrictions discoverable when those options are closed, including a tag-only search.
Distinguish draft search options from the successful search that produced the displayed files; failed searches must not relabel those results.
Use descriptive retrieval labels and compact action icons with hover hints where the action remains clear.
Align each input's visible height with adjacent compact buttons.
Use suitable GPUI Kit components for the desktop's controls and interaction.
Keep knowledge-folder selection and initialization in an on-demand panel because they are infrequent operations.
Place the next-refresh semantic-index option in that panel and explain that it neither starts processing nor indicates service availability.
Keep adding files beside the tree and index refresh beside index status; do not duplicate index refresh in the search toolbar.
Show unfiltered files in a collapsible directory tree, with selectable leaves and visible source identity.
For an active search, preserve the retrieval order in ranked file rows instead of regrouping files by directory.
Group a file's additional evidence beneath its best returned evidence, and distinguish semantic candidates from exact matches.
Clicking line evidence must reveal and highlight its numbered text location; retain formatted document reading separately.
Preserve search settings, results, and the selected document when opening and closing the guide.
Keep metadata editing available without reserving a large permanent toolbar row.
Save editable metadata directly without starting an index build, and show whether the write succeeded.
Present file details with labelled type, readable size, local creation and modification dates, and explicit path-copy actions.
Show an unavailable date honestly instead of substituting another timestamp.
Label editable tags and notes separately from source properties and explain that they are saved with the knowledge library without changing the source.
Identify the saved record's location, make it inspectable, and reload the saved values when the file is selected again.
Explain that an explicit index refresh makes new tags and notes available to search.
Distinguish unsaved changes, saving, saved values, and failures, including edits that return to their saved value.
Keep preview selection, typing, and metadata inspection responsive while searches or explicit index builds run.
Read selected-file attributes independently of the complete search index.
Keep the bottom status bar to one compact row during normal browsing.
Show the transfer destination and actionable operation feedback in that row; omit routine preview-ready and file-row instruction messages.
Show complete contextual hints only while hovering, and release any temporary hint space when the pointer leaves.
Remove permanent instructional and destination rows below the file tree and duplicate feedback beside the retrieval controls.
Keep status-bar diagnostics readable within its layout instead of floating under the native preview.
Show selected-folder, index, external-tool, and local-model-service status in a compact bottom bar.
Link dependency problems to their installation or repair instructions.
Distinguish uninitialized folders, invalid selections, missing indexes, and source changes that need reindexing.
Check these states in the background without automatically initializing, indexing, installing, or starting a service.
Provide source-file context actions for copying the file, revealing it in Finder, copying absolute or relative paths, and moving it to the macOS Trash.
Right-clicking a tree row must select its actual target and open its menu on the first click, including when another row's menu is open.
Dismiss that menu when the user clicks outside it, including inside the native preview, or presses Escape.
Require an explicit file action and confirmation before moving a source to Trash; preserve its authored metadata for recovery.
Group the current knowledge root's name and location in one bottom-bar target that opens its folder.
Place index freshness beside the index-refresh action at the right end of the bar, separate from library identity.
Clicking a stale or missing-index state for the opened root must start an explicit index refresh, rather than open documentation.
If the edited path identifies a different knowledge root or an uninitialized folder, direct the user to open or initialize it before offering indexing.
Never refresh the previously opened library by mistake.
Treat search as a clearable filter over the file tree and restore the complete browsing view when the filter is cleared.
Support explicit file drag-and-drop, copy, and paste within the desktop; preserve metadata and reject overwrite collisions.
Store derived indexes inside the knowledge folder and keep upgraded indexes usable when the whole library moves.
Use native text inputs with consistent focus, Chinese input-method composition, selection, clipboard commands, and standard word-editing shortcuts.
Open explicit Markdown and HTML links through safe routes for document anchors, contained library files, and the system browser.
Provide default-application, Finder, Quick Look, and metadata actions for the selected source.
Show an explicit unsupported-preview message for formats outside the embedded renderer's coverage, including RAF camera files.
Keep unsupported files available through metadata search and native macOS viewers.

## Source Authority And Effects

Original files and user-edited Markdown or metadata remain authoritative.
Extracted text, search indexes, and vectors are derived data and must be rebuildable.
Import copies sources and never deletes the source.
Initialization must preserve existing content and report collisions.
Search results must identify the source and return line numbers or media time ranges where available.
Local processing is the default; no file content leaves the machine without explicit authorization.
Do not execute scripts from imported content.

## Implementation Defaults

Use `omni-knowledge.json` as the root marker and configuration, `.omni/metadata/` for editable JSON asset descriptions, and `.omni/indexes/` for generated search snapshots.
Associate each file with its nearest root; parent operations exclude nested roots unless recursive search is requested.
Use Tantivy BM25, parallel regex verification, conservative literal trigram candidate filtering, and exact local cosine scoring.
Publish a completed index generation through an atomic pointer and retain the previous published generation after failed work.
Bound extraction, report unsupported or failed extraction explicitly, and still index file names and metadata.
Semantic search requires an available compatible local model; lexical search works independently.

## Acceptance Examples

Initialize a nonempty folder without overwriting its existing files.
Import an unknown binary file, annotate it, and retrieve it by exact tag and description.
Retrieve English and Chinese Markdown content through BM25 and grep, then show the original file.
Index modified and deleted files without returning removed content after successful reconciliation.
Keep parent and child library identity distinct and return both through recursive search.
Retrieve an image and a timed video segment with a natural-language query using the local WeMM service.
Create an EPUB with readable navigation and chapters from Markdown and HTML, then reopen it.
Report missing parser or embedding dependencies without claiming content extraction or semantic coverage.
