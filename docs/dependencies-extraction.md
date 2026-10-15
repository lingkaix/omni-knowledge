# Extraction And EPUB Dependencies

This guide records local parser choices and their limits for [product requirements](specs/product.md).
The source review date is 2026-10-14.

## Selected Libraries

| Library | License | Purpose | Source |
| --- | --- | --- | --- |
| `pdf-extract` 0.12.1 | MIT | Native PDF text extraction without a separate runtime. | [Repository](https://github.com/jrmuizel/pdf-extract) |
| `html2text` 0.16 | MIT | HTML text extraction and parsed document trees. | [API](https://docs.rs/html2text/0.16.7/html2text/) |
| `ammonia` 4 | MIT OR Apache-2.0 | Remove executable HTML, unsafe attributes, and unsafe URLs. | [Repository](https://github.com/rust-ammonia/ammonia) |
| `pulldown-cmark` 0.13 | MIT | Render Markdown tables, code fences, and common extensions. | [Repository](https://github.com/pulldown-cmark/pulldown-cmark) |
| `roxmltree` 0.21 | MIT OR Apache-2.0 | Read Office and EPUB XML without external entity retrieval. | [API](https://docs.rs/roxmltree/0.21.1/roxmltree/) |
| `zip` 4 | MIT | Read archive members in memory and create EPUB containers. | [Repository](https://github.com/zip-rs/zip2) |

The extractor reads Markdown and other UTF-8 text directly.
It reads DOCX text runs, PPTX slides, XLSX shared strings and cells, and OpenDocument paragraphs from their native containers.
It uses the existing macOS `textutil` executable for legacy DOC and RTF.
The executable runs without a shell, with a 30-second limit and a bounded output pipe.
Each archive uses a shared expanded-byte limit, and the software never unpacks archive member paths onto the filesystem.

Office extraction returns text for search and a simple preview; it does not reproduce page layout, charts, formulas, or spreadsheet styling.
PPTX and XLSX extraction use numeric member order; custom package ordering and hidden-slide or hidden-sheet state are not reproduced.
PDF extraction reads the native text layer; scanned pages require an optional OCR implementation.
Encrypted, malformed, oversized, or unsupported files retain metadata search coverage with an explicit extraction status.

## Evaluated Alternatives

[MarkItDown](https://github.com/microsoft/markitdown) is MIT-licensed and converts Office, PDF, HTML, EPUB, images, and audio through Python and optional dependencies.
Its Markdown output targets text analysis, and it does not promise faithful document layout.
The first version uses native Rust parsers to avoid a mandatory Python installation.
An optional local converter can be added when a user needs stronger layout conversion or additional formats.

[LiteParse](https://github.com/run-llama/liteparse/tree/main/crates/liteparse) now provides a Rust library and `lit` CLI under Apache-2.0.
Its current Rust core uses PDFium, offers Markdown and structured coordinates, and supports macOS ARM.
The default build includes Tesseract; Office conversion requires LibreOffice.
LiteParse is a suitable future PDF and OCR adapter when its native build and packaging costs are justified.
The evaluated version is 2.15.1; benchmark results in its README are upstream evidence and were not reproduced here.

[Univer](https://github.com/dream-num/univer) provides an Apache-2.0 Office editor SDK for browser and Node.js integrations.
Its documented Office import and export capabilities belong to the Pro commercial layer.
The first version therefore uses local text previews and macOS native viewing instead of claiming free native Office rendering through Univer.

[Calamine](https://github.com/tafia/calamine) is an MIT-licensed Rust reader for XLS, XLSX, XLSB, and ODS.
It is a future replacement for the narrow XLSX reader when wider spreadsheet fidelity is needed.
The first version reports unsupported binary spreadsheet extraction instead of interpreting XLS bytes as text.

The [`epub` reading crate](https://github.com/danigm/epub-rs) uses GPL-3.0-or-later.
The [`epub-builder` export crate](https://github.com/crowdagger/epub-builder) uses MPL-2.0.
This implementation reads and writes the required EPUB container structures directly through the selected ZIP and XML libraries.
The format owner is the [W3C EPUB 3.3 specification](https://www.w3.org/TR/epub-33/).

## EPUB Scope

Reading follows the package spine, resolves percent-encoded resource paths, and supports multilingual UTF-8 XHTML.
Encrypted chapter content returns an explicit error; standard font obfuscation does not prevent text reading.
The simple preview shows chapter text and does not reproduce fixed layouts, styles, or embedded media.

Export includes Markdown and HTML chapters with navigation, required metadata, XML-compatible content, and an uncompressed first `mimetype` member.
The exporter excludes `.omni`, `.git`, symbolic-link entries, and nested knowledge-library roots.
When media is requested, it includes referenced local images, audio, and video that remain inside the same library.
Media references cannot pass through symbolic links or include internal `.omni` and `.git` files.
It preserves source files, validates chapter XML, stages output locally, and refuses to overwrite an existing output path.
Each chapter has a 64 MiB input limit, each media file has a 128 MiB limit, and aggregate source content has a 512 MiB limit.
Advanced typography, fixed layouts, remote media downloads, digital rights management, and EPUB accessibility certification are outside the first version.
The local tests verify readable chapters and container structure; they do not establish complete EPUBCheck conformance across readers.

## Preview Safety

Markdown raw HTML is displayed as inert text.
HTML sanitization removes scripts and event attributes while retaining common document structure.
Relative media references remain available to the GUI's contained local protocol, and network image references are removed.
The GUI owns the content security policy and serves local files only after root containment checks.
Front matter remains visible, and fenced language classes support syntax highlighting and Mermaid rendering by trusted GUI assets.
