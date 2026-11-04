# Omni Knowledge

Omni Knowledge is a local folder knowledge library with an Apple silicon GPUI desktop and a headless Rust CLI.
Original files stay in ordinary folders; generated search indexes can be rebuilt.

## Full-Version Development

The accepted [product contract](docs/specs/product.md) defines one full delivery with shared portable crates, reference mounts, safe text history, searchable ZIP packages, and a common multi-library service.
The [service wire contract](docs/specs/service.md) defines the public DTO boundary for GUI, CLI, and MCP.
The [workspace transition](docs/changes/2026-11-04-workspace.md) records migration and preservation obligations.
The [headless-first amendment](docs/changes/2026-11-04-headless-delivery.md) prioritizes the complete headless service, MCP, and CLI mechanisms for the current delivery.
GUI milestone M7 is deferred; the existing GPUI application keeps building and adapts only where a service DTO change requires compatibility.
These accepted contracts are not claims that every feature is already implemented or qualified.
The compatibility commands and storage description below describe the retained first-version implementation while the migration is in progress.
Linux headless builds and macOS GUI builds have separate gates; native platform effects require their actual platform observations.

## Try The Retained First Version

Build and open the local application:

```sh
sh scripts/bundle-macos.sh
open "dist/Omni Knowledge.app"
```

Open **Library**, then select **Choose** for an existing knowledge folder or enter a path and select **Initialize**.
Use **Refresh index** after an agent edits files.
Select a result to preview it; **Open original** opens the source in its installed application.
See [the desktop guide](docs/gui.md) for controls and preview limits.
The main workspace keeps one search row at the top, a collapsible directory tree for browsing, and ranked file results during search.
Enable the compact search-method chips and enter comma-separated keywords in one field, including exact tags.
Search ignores case by default, matches any keyword through any enabled method, and clears inside the search field.
**File details** shows readable file properties, copyable paths, and labelled tags and notes saved with the library.
The build script applies and verifies a local ad-hoc signature; it does not create a notarized distribution package.
The app includes the core search and document parsers, but video processing, embeddings, and ASR still require external tools or models.
Open **说明与环境** for the built-in usage guide, dependency checks, and Homebrew installation buttons.
See [runtime portability](docs/dependencies.md#runtime-portability) before moving it to another Mac.

The prepared test library is at `temp/demo-knowledge/` in this checkout.
It includes Chinese Markdown, HTML, PDF, DOCX, EPUB, binary, image, video, and spoken-audio fixtures.
The temporary demo is not a replacement for your own sources.

## CLI

```sh
cargo build --release --bin omni
./target/release/omni init /absolute/path/to/knowledge --name "Personal Knowledge"
./target/release/omni --root /absolute/path/to/knowledge add /absolute/path/to/file.pdf --tag research
./target/release/omni --root /absolute/path/to/knowledge search "knowledge retrieval"
./target/release/omni --root /absolute/path/to/knowledge search "知识检索" --mode grep --json
./target/release/omni --root /absolute/path/to/knowledge metadata raw/file.pdf --tag research --description "Experiment report"
./target/release/omni --root /absolute/path/to/knowledge index
./target/release/omni --root /absolute/path/to/knowledge export-epub /absolute/path/to/book.epub --include-media
```

Use `--json` for structured agent output; errors return a nonzero exit code.
CLI tag filters match complete values without case sensitivity by default, and repeated filters require every tag.
`search --mode files` includes unsupported binary formats through names and descriptions.
`search --recursive` combines independently indexed nested libraries and returns their root identities.
Run `omni --help` and a subcommand's `--help` for the complete interface.

## Search And Formats

| Capability | First-version behavior |
| --- | --- |
| Tags | Exact filters from YAML front matter and editable JSON metadata |
| Grep | Parallel regex verification over extracted index snapshots; eligible literals use trigram candidates |
| BM25 | Tantivy with English words and Chinese character/bigram tokens |
| Multimodal | Local WeMM text/image queries, image vectors, and timed ffmpeg video clips |
| Audio | Imported timed transcripts or configured local whisper.cpp speech recognition |
| Markdown / HTML | Safe preview, front matter, offline Mermaid, and code highlighting |
| PDF | Native text extraction, WebKit preview attempt, native application fallback |
| Office | Native DOCX/PPTX/XLSX and ODF extraction; DOC/RTF through macOS textutil; native application fallback |
| EPUB | Ordered chapter preview, Markdown/HTML EPUB3 export, optional contained media |
| Other files | General filename, size, MIME type, SHA-256, tags, and description indexing |

Text extraction is bounded to 8 MiB of input and derived text per source in this version.
Larger or unsupported sources remain metadata searchable and retain the original file.
OCR is deferred; image-only PDFs need a separate OCR pass before text search can find their content.
Office preview shows extracted content rather than editable spreadsheet or presentation layouts.
EPUB preview does not reproduce fixed-layout publisher styling or interactive content.
Search uses the last successful snapshot; it does not automatically watch filesystem changes yet.
The desktop periodically checks source metadata and prompts for index refresh; it does not rebuild automatically.
All regular file types are admitted; source symlinks and `.git`/`.omni` internals are excluded.
Non-UTF-8 file names produce an explicit indexing error in this first version.

## Local Models

Use **说明与环境** to install FFmpeg or whisper.cpp through Homebrew.
The app checks standard Homebrew paths even when launched from Finder.
Speech models and the WeMM provider remain separate setup steps; installation does not change a knowledge root's configuration.

The default configuration targets the existing local WeMM provider on `http://127.0.0.1:18789`.
The provider is a separate process and must be running for vector indexing or queries.
Lexical search works while it is stopped.

```sh
cd /absolute/path/to/embedding-wemm
./scripts/serve.sh gpu
```

Then run from this repository:

```sh
./target/release/omni --root /absolute/path/to/knowledge doctor --embeddings
./target/release/omni --root /absolute/path/to/knowledge index --embeddings
./target/release/omni --root /absolute/path/to/knowledge search "a performance comparison table" --mode semantic
./target/release/omni --root /absolute/path/to/knowledge search --mode semantic --image /absolute/path/to/query.jpg
```

Read [the multimodal guide](docs/multimodal.md) for provider identity, ASR setup, and model limits.
The first version accepts only loopback embedding endpoints and never uploads library content to a remote model.

## Agent Interface

Start the stdio MCP server with:

```sh
./target/release/omni --root /absolute/path/to/knowledge serve-mcp
```

It exposes `search`, `read`, `list`, `status`, `index`, `import`, and `metadata` tools.
Use [the agent guide](docs/agents.md) for a client configuration and workflow.
An external AI agent maintains synthesis; the software does not claim to autonomously reason about sources.

## Library Structure

Initialization creates `omni-knowledge.json`, `raw/`, `wiki/`, navigation, history, and an agent guide.
The two requested foundational articles are preserved as original offline text snapshots in `raw/` and linked from OKF reference concepts.
[Template provenance](assets/foundations/README.md) records source URLs and hashes.
Each root owns files until a child root begins; an import cannot write into a child-owned `raw/` folder.
An asset named `omni-knowledge.json` is imported as `omni-knowledge.asset.json` to preserve root detection.

Editable metadata lives in `.omni/metadata/` and must be included in a backup alongside original files and the root configuration.
Indexes live in `.omni/indexes/`; publication retains the current and previous completed generations.
Unknown retained files in an old generation are preserved and reported instead of being deleted.
See [the storage specification](docs/specs/storage.md) for ownership and recovery.

## Develop

Read [AGENTS.md](AGENTS.md), [the documentation map](docs/README.md), and [CONTRIBUTING.md](CONTRIBUTING.md).
`mise.toml` selects Rust 1.99.0; an existing compatible Rust toolchain also works.
GPUI uses runtime Metal shaders, so no separate Metal compiler download is required.

```sh
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo fmt --all -- --check
```

The embedding tests create isolated loopback listeners and need local socket permission in restricted runners.
ffmpeg, ffprobe, and macOS textutil are exercised by the relevant local format tests.
[Dependency choices](docs/dependencies.md), [decisions](docs/decisions/README.md), and [the first-version record](docs/changes/2026-10-14-first-version.md) state evidence and remaining limits.

## License

Original project code uses the [Apache License 2.0](LICENSE), as selected by the engineer.
Third-party components and the foundational source articles retain their original licenses and provenance.
See [dependency licensing](docs/dependencies.md#ffmpeg-packaging-boundary) for the FFmpeg process boundary and future redistribution requirements.
