# Native Desktop Interface

The desktop interface uses GPUI 0.2.2 for native controls and Metal rendering.
GPUI Kit 0.5.1 provides controls, native text inputs, menus, and the virtualized tree on the same GPUI version.
It embeds a WKWebView through Wry 0.57.0 for document previews.
The shared Rust library owns initialization, indexing, import, metadata, and retrieval behavior.

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
3. Select a destination folder and **Import** to copy files there without replacing an existing file; no selection uses the root.
4. Select **Refresh index** after external file changes.
5. Enter a query, select a retrieval mode, and press Enter or select **Search**.
6. Select a result to inspect its preview and source path.

The tag filter uses comma-separated exact values.
Every entered tag must match.
An empty query browses files with the current tag filters.
The browsing tree reads live directories, including empty folders and files that have not been indexed.
Search shows up to 10,000 hits from the last published snapshot.
Clear the filter to restore directory browsing.

The selected file has separate inputs for editable tags and a description.
Select **Save metadata** to write its sidecar without starting indexing.
Refresh the index explicitly when the saved tags and description should appear in retrieval.
Source front matter remains part of the combined tag set.
The description is useful for binary files that have no extractable text.

## Workspace

The compact top toolbar keeps search, a mode dropdown, and exact-tag filtering accessible.
Hover over an action icon to read its hint; sidebar hints stay within the file pane.
The **Library** panel contains folder selection and initialization and is collapsed during normal reading.
The left pane groups files by directory; expand or collapse a folder to control the visible tree.
Use a file's context menu to open it, reveal it in Finder, copy it, copy its absolute or relative path, inspect metadata, or move it to Trash after confirmation.
Paste copied files into a folder, drop external files to copy them, or drag an owned file to move it within the library.
Transfers reject destination collisions and retain authored metadata; they do not silently refresh the search index.
File selections keep their source path and available line or media-time evidence.
Use **Metadata** beside the selected-file preview to open its tags and description editor.
Click the green logo to return from the guide without clearing the query, results, or selected preview.

## Guide And Environment

Select **说明与环境** to read the in-app Chinese guide and check external tools.
The guide covers initialization, imports, exact tags, each search mode, previews, metadata, CLI/MCP use, backup, and current limits.
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

Select **Embeddings: on** before refreshing the index to generate semantic vectors through the configured local provider.
Semantic and hybrid retrieval require a working provider and indexed vectors.
The status bar shows provider and extraction failures.
Filesystem and model work runs outside the UI thread.
Preview selection, search, metadata saving, and explicit indexing use separate background tasks.
Opening an existing library and selecting files do not load its full search snapshot or start an index build.
Rapid selections replace pending preview requests; an obsolete completion cannot replace the current selection.
The bottom bar shows the opened library location and checks the selected folder, index metadata, installed tools, provider identity, and configured ASR files.
Click the location to open its folder in Finder, or click a problem badge to read repair instructions.
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
The metadata inspector shows live path, MIME type, size, modification time, and editable notes without reading source contents or the complete index.
Selecting a timed audio or video result starts its preview at the returned evidence timestamp.
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
