# Native Desktop Interface

The desktop interface uses GPUI 0.2.2 for native controls and Metal rendering.
GPUI Kit 0.5.1 provides controls, native text inputs, menus, and the virtualized tree on the same GPUI version.
It embeds a WKWebView through Wry 0.57.0 for document previews.
The shared Rust library owns initialization, indexing, import, metadata, and retrieval behavior.
The 2026-11-04 headless-first amendment defers full-version GUI feature work.
This guide documents the retained application, which keeps building and adapts only where a service DTO change requires compatibility.
See [the headless-first amendment record](changes/2026-11-04-headless-delivery.md) for the current delivery priority.

## Run

```sh
cargo run --features gui --bin omni-knowledge-gui
cargo run --features gui --bin omni-knowledge-gui -- /absolute/path/to/knowledge
```

The GUI targets Apple silicon macOS.
Xcode command line tools provide the macOS SDK.
GPUI compiles Metal shader source at runtime through its `runtime_shaders` feature, so the optional offline Metal compiler is unnecessary.

To create a local application bundle, run:

```sh
sh scripts/bundle-macos.sh
open "dist/Omni Knowledge.app"
```

The bundle includes both the GUI and CLI executable.
Its soft-green logo and macOS icon use the engineer's supplied vector design; [brand assets](../assets/brand/README.md) record the source and palette.
The script applies a local ad-hoc signature after assembling the complete bundle and verifies it before replacement.
It preserves the previous bundle at the path printed by the script.
It does not use a Developer ID certificate, notarize, install, or publish the application.
Read [the dependency guide](dependencies.md#runtime-portability) before copying the application to another Mac.

## Use

1. Open **Library**, then enter a folder path or select **Choose**.
2. Select **Open** for an existing knowledge root, or **Initialize** to create the template and first lexical index.
3. Select a destination folder and **Add files** beside the tree to copy files there without replacing an existing file; no selection uses the root.
4. Select **Refresh index** in the bottom status bar after external file changes.
5. Enter a query and press Enter or select **Search**; enable the compact method chips for the same comma-separated keywords.
6. Select a result to inspect its preview and source path.

Enable **Full text**, **Text**, **Meaning**, **Files**, and **Tags** independently below the query field.
Full text is enabled by default; click a chip to include or exclude that method from the next search.
ASCII and Chinese commas separate keywords; empty entries and case-insensitive duplicates are ignored.
Any keyword through any enabled method can retrieve a file.
Tags matches complete tag values without case sensitivity and uses the same query field.
For example, enable only Tags and enter `research, travel` to find either complete tag.
Text searches literal text without case sensitivity; regex punctuation remains literal in the desktop.
The CLI retains its existing regex mode and required-tag filters.
An empty query restores live browsing, and a nonempty query requires at least one enabled method.
A selected Meaning method requires a compatible local provider; a method failure retains the previous successful results and their conditions.

The browsing tree reads live directories, including empty folders and files that have not been indexed.
Search retains up to 10,000 evidence records from the last published snapshot.
Active searches show ranked file rows rather than directory groups.
Images have one leaf row, without separate metadata and visual-vector children.
Text results expand only for distinct line locations; recordings expand only for distinct time ranges.
Repeated locations and unlocated metadata do not appear as extra navigation targets.
The first page shows 50 distinct files; **Show 50 more files** reveals the next page in the same order.
Results involving Meaning use **candidates**, because vector similarity has no calibrated match threshold.
Hover over the file count to inspect distinct files, useful locations, retained retrieval records, and any result-limit notice.
Use the clear button inside the search field to clear keywords and restore directory browsing without changing enabled methods.

Open **File details** for the selected file's labelled **Tags** and **Notes** fields.
Select **Save tags and notes** to write a separate record with the knowledge library, without changing the original or starting indexing.
Notes support multiple lines; returning to the saved values clears the unsaved-change indicator.
Save before selecting another file; unsaved drafts are not retained between files.
The editor identifies the record location and provides an action to inspect an existing saved record in Finder.
Select the file again and reopen **File details** to read the saved tags and notes.
Refresh the index explicitly when the saved tags and description should appear in retrieval.
The editor shows only tags saved in that separate record; source front matter remains unchanged and joins the combined tag set during indexing.
The description is useful for binary files that have no extractable text.

## Workspace

The single top search row contains the query, inline clearing, and Search.
A compact row of method chips keeps enabled retrieval methods visible without a second query field or dropdown.
Press Enter or select **Search** to apply edited keywords and methods; unapplied changes remain distinguishable from the displayed results.
A failed search retains the last successful view and its actual conditions.
Descriptive chip labels identify full text, literal text, meaning, file names and notes, and exact tags without requiring algorithm names.
The bottom status bar normally occupies one row, with the library name and path, transfer destination, environment observations, and index controls.
Hover over an action icon or status item to show its full hint temporarily; leaving it restores the single-row layout.
The library name and path share one Finder action, and routine preview-ready and file-row instructions are omitted.
The **Library** panel contains folder selection, initialization, and the next-refresh semantic-index option and is collapsed during normal reading.
**Add files** belongs to the file-tree toolbar; index refresh appears only beside index status in the footer.
During browsing, the left pane groups live files by directory; expand or collapse a folder to control the visible tree.
During search, the same pane preserves returned file rank and collapses distinct line or time locations under each source.
Use a file's context menu to open it, reveal it in Finder, copy it, copy its absolute or relative path, inspect metadata, or move it to Trash after confirmation.
Right-click selects the menu's target directly; clicking the preview or elsewhere outside the menu dismisses it, as does Escape.
Paste copied files into a folder, drop external files to copy them, or drag an owned file to move it within the library.
Transfers reject destination collisions and retain authored metadata; they do not silently refresh the search index.
File selections keep their source path and available line or media-time evidence.
Use **File details** beside the selected-file preview to inspect source properties and edit its tags and notes.
Path-copy actions copy the complete absolute or relative path, including characters truncated in the display.
Sizes use readable units with an exact byte count, and creation and modification dates use labelled local times.
Unavailable dates remain explicit rather than borrowing another timestamp.
Click the green logo to return from the guide without clearing the query, results, or selected preview.

## Guide And Environment

Select **说明与环境** to read the in-app Chinese guide and check external tools.
The guide covers initialization, imports, exact tags, enabled search methods, previews, metadata, CLI/MCP use, backup, and current limits.
The environment controls check Homebrew, FFmpeg, ffprobe, and whisper-cli without installing anything.
Select **在终端安装 FFmpeg** or **在终端安装 whisper.cpp** to start the fixed Homebrew command.
Terminal shows progress, output, and the command's exit result; Omni Knowledge reports that it handed off the command.
The visible Terminal installation continues independently if Omni Knowledge closes.
After installation completes, select **重新检查** to confirm executable availability.
If Homebrew is absent, use the official installation page linked by the guide.
Models and library ASR/provider configuration remain separate explicit setup steps.
Standard Homebrew paths work without an interactive shell's PATH.
Use **返回知识库** to return to the existing results and preview.

## Semantic Indexing

Enable **Build semantic search index** in **Library** before refreshing the index to generate semantic vectors through the configured local provider.
This per-window option applies to the next explicit refresh; it does not start the model service or persist a new library preference.
Disabling generation does not remove every existing unchanged vector.
Semantic and hybrid retrieval require a working provider and indexed vectors.
The status bar shows provider and extraction failures.
Filesystem and model work runs outside the UI thread.
Preview selection, search, metadata saving, and explicit indexing use separate background tasks.
Local filename, tag, text-pattern, and full-text searches can run while an obsolete semantic request finishes.
Each local or model search lane has one active worker, with only the newest pending query retained across both lanes.
Only the latest root and query can replace the displayed results; clearing search drops pending work without claiming to cancel a blocking model request.
Opening an existing library and selecting files do not load its full search snapshot or start an index build.
Rapid selections replace pending preview requests; an obsolete completion cannot replace the current selection.
The bottom bar checks the selected folder, index metadata, installed tools, provider identity, and configured ASR files.
Click the library name or path to open its folder in Finder.
At the right end, click a stale or missing-index state to refresh the opened library's index, or use **Refresh index** beside it.
Dependency badges open the relevant installation or repair instructions.
If the Library input refers to a different unopened library, the index area directs you to open it first and cannot refresh the previous library by mistake.
An uninitialized folder remains unchanged until **Initialize** is selected.
The quick freshness check compares names, sizes, and timestamps; explicit indexing performs content-hash reconciliation.
If opening a Documents, Desktop, or Downloads folder waits for access, complete the corresponding macOS file-permission prompt.
Local ad-hoc builds can require renewed consent after the executable changes.

## Preview Coverage

Markdown previews render front matter, tables, code highlighting, and Mermaid diagrams.
HTML previews use sanitized document content.
The Mermaid and highlight.js runtimes are bundled for offline use.
Their versions and licenses are recorded in [the vendor guide](../assets/vendor/README.md).

Images, supported audio, and supported video use WKWebView media elements.
Unsupported image formats such as RAF show a clear coverage message and remain available through **Quick Look** or **Open original**.
The file-details inspector shows live paths, MIME type, readable size, creation and modification dates, saved-record location, tags, and notes without reading source contents or the complete index.
Selecting a timed audio or video result starts its preview at the returned evidence timestamp.
Selecting line evidence opens numbered text and centers the highlighted line; selecting another occurrence of the same file changes that location.
Plain text has a left number gutter; Markdown retains original source coordinates, including front matter.
For Markdown, HTML, and EPUB, close the numbered-text disclosure to return to formatted reading.
HTML, EPUB, PDF, and Office lines describe the extracted text used by the index, rather than page or source-markup positions.
If the current text has no requested line, the preview reports that stale evidence instead of highlighting a substitute.
Numbered previews contain at most 20,000 rows, centered around a distant requested line, with an explicit window notice.
PDF previews attempt the native WebKit viewer and include extracted text.
If the embedded PDF viewer is unavailable, expand **Extracted text** or select **Open original**.
EPUB and Office files use the available safe extraction preview.
Select **Open original** to use the installed native application for a format or codec that WebKit cannot display.

Individual preview resource reads are limited to 32 MiB.
Media byte-range requests receive bounded partial responses, so large media can load in multiple requests.
Large resources requested without byte ranges remain searchable and can be opened in a native application.
EPUB previews show extracted chapters; the first version does not reproduce publisher styling or interactive content.
The GUI does not yet expose image-query selection or EPUB export controls; the CLI owns those operations.

## Preview Boundary

The custom `omni:` protocol serves embedded application scripts and files inside the selected root.
Each opened root has its own resource origin; obsolete resource and navigation URLs cannot read a newly opened root.
Canonical path checks reject traversal and symbolic links that escape the root.
The preview has no agent or filesystem mutation interface.
Its content security policy blocks remote resources, network requests, frames, and source scripts.
Only the embedded renderer scripts execute.
Explicit source-link clicks use safe routes for heading anchors, contained library files, and HTTP or HTTPS links in the system browser.
Other schemes, credentials, and paths outside the root are rejected.

## Verification

The GUI module has focused tests for resource confinement, media byte ranges, Unicode file URLs, and preview policy headers.
Native rendering, input methods, and WebKit media behavior require an actual macOS window check.
Build success and resource tests alone do not establish the native layout or codec coverage.

On 2026-10-14, the release window was operated on the Apple M5 Pro.
The check covered Chinese queries, front matter, Mermaid, code highlighting, native PDF, extracted DOCX and EPUB, image/video retrieval, video playback, and audio starting at returned evidence time.
Binary metadata edits were verified through the CLI and exact-tag retrieval.
See [the first-version record](changes/2026-10-14-first-version.md) for commands, evidence, and remaining limits.
The [desktop workspace record](changes/2026-10-15-desktop-workspace.md) covers the later input, file management, responsiveness, metadata, status, and signed-bundle checks.
The [status and menu record](changes/2026-10-15-status-and-menus.md) covers the compact footer, direct index actions, hover transitions, and native menu boundary.
The [usability record](changes/2026-10-15-workspace-usability.md) covers contextual search options, applied-filter identity, readable file details, and authored-record persistence.
