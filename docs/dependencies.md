# Dependencies And Runtime Setup

Original project code uses [Apache License 2.0](../LICENSE), selected by the engineer.
Third-party components and source articles retain their own licenses.
`Cargo.lock` fixes Rust dependency versions.

## Included Components

| Component | Purpose | License |
| --- | --- | --- |
| Tantivy | BM25 and exact tag indexing | MIT |
| GPUI | Native controls and Metal rendering | Apache-2.0 |
| GPUI Kit 0.5.1 | Inputs, controls, menus, and tree on GPUI 0.2.2 | Apache-2.0 |
| Wry | macOS WebKit preview | MIT / Apache-2.0 |
| clap, serde, regex, rayon | CLI, structured data, and text search | MIT / Apache-2.0 |
| fs2, tempfile, sha2, reqwest, url | Storage, fingerprints, and local model requests | MIT / Apache-2.0 |
| Mermaid, highlight.js | Offline diagrams and code highlighting | MIT / BSD-3-Clause |
| Lucide / Feather icons in GPUI Kit assets | Native action icons | ISC / MIT |

[Extraction dependencies](dependencies-extraction.md) cover the PDF, Office, EPUB, Markdown, and HTML parsers.
[Renderer assets](../assets/vendor/README.md) retain their licenses and provenance.
The bundle includes Rust dependency notices and exact unmodified MPL-2.0 source archives under `Contents/Resources/ThirdParty/`.
The locked Darwin graph includes MPL-2.0 components such as cssparser, dtoa-short, and option-ext; their terms remain separate from the project's Apache license.
The generated inventory states notice-coverage gaps instead of treating SPDX metadata as a complete redistribution audit.
The compiled GUI and CLI link only macOS system libraries.
Basic search and document preview need no separate Rust, Node.js, Python, or parser installation.

## External Tools

The engineer selected Homebrew setup from the app's guide page instead of bundling external media tools.
An explicit button click opens the fixed install command in Terminal, where progress, output, and interaction remain visible.
The app reports handoff rather than installation success; use **重新检查** after Terminal completes.
If Homebrew is absent, the page links to its official installation instructions.
Tool discovery includes standard Homebrew locations so Finder launches can find installed tools.

| Feature | Requirement | Setup |
| --- | --- | --- |
| Video indexing and audio conversion | FFmpeg and ffprobe | Terminal install button runs `brew install ffmpeg` |
| Local speech recognition | whisper.cpp and a speech model | Terminal install button runs `brew install whisper.cpp`; configure a local model separately |
| Semantic and multimodal retrieval | Compatible WeMM provider and complete model package | Follow [local model setup](multimodal.md) |
| Legacy DOC/RTF | macOS textutil | Provided by macOS |
| OCR | Not implemented | Deferred |

## Runtime Portability

Model packages are not downloaded automatically, and installation does not change a library's configuration.
The app is not a complete all-feature offline package.
The M3 machine remains untested.
Local ad-hoc signing verifies bundle integrity and does not provide Developer ID trust or notarization; see [the GUI guide](gui.md).
Move the whole library, including `.omni/metadata/` and `.omni/indexes/`, to preserve its authored metadata and derived indexes.
Upgraded roots retain index identity across relocation; legacy roots need one explicit index build to obtain a stable identity.
Update machine-specific model paths and reindex when sources or model identity change.

## FFmpeg Packaging Boundary

FFmpeg runs as a separate program through command-line arguments and ordinary media files; the app does not link its libraries.
This supports keeping original project code under Apache 2.0 while FFmpeg retains its own terms, consistent with [GNU aggregate guidance](https://www.gnu.org/licenses/gpl-faq.en.html#MereAggregation).
FFmpeg builds can use LGPL or GPL depending on enabled components; they cannot be relabelled as Apache-licensed.
A future decision to distribute its binaries must satisfy the applicable notices and corresponding-source obligations described by [FFmpeg](https://ffmpeg.org/legal.html).
The current decision uses Homebrew installation and includes no FFmpeg, whisper.cpp, WeMM executable, or model weights in the app.
