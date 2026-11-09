# Omni Knowledge

Omni Knowledge keeps original files in ordinary folders and returns source-linked search results through a headless CLI and MCP.
The first usable version is qualified on Linux x86_64 from code commit `dfa0902`.
It covers lexical/BM25 search, Japanese BM25, DOCX extraction, and canonical and legacy MCP.
See the [qualification summary](first-usable-version.md) for the exact artifact and evidence.

## Start On Linux

Use `omni-knowledge-linux-x86_64-n02-dfa0902.tar.gz` supplied with this version.
Its SHA-256 is `1e9c56156cc316ee35497f6c62369cf3c900ea195bdf8cef246abb1d462175f8`.
Keep the complete extracted directory, including `parser-worker`, licenses, and notices.

```sh
set -e
OMNI_ARCHIVE=/absolute/path/to/omni-knowledge-linux-x86_64-n02-dfa0902.tar.gz
printf '%s  %s\n' '1e9c56156cc316ee35497f6c62369cf3c900ea195bdf8cef246abb1d462175f8' "$OMNI_ARCHIVE" | sha256sum --check
mkdir -p "$HOME/.local/opt"
OMNI_INSTALL="$HOME/.local/opt/omni-knowledge-dfa0902"
mkdir "$OMNI_INSTALL"
tar -xzf "$OMNI_ARCHIVE" -C "$OMNI_INSTALL" --strip-components=1
OMNI_BIN="$OMNI_INSTALL/omni"
"$OMNI_BIN" --help
```

The block stops on a checksum mismatch or an existing installation directory.
Initialize a new library and add a small source:

```sh
OMNI_LIBRARY=/absolute/path/to/new-library
"$OMNI_BIN" init "$OMNI_LIBRARY" --name "My Knowledge"
printf '%s\n' '---' 'tags: [research]' '---' '# Hello' 'Hello knowledge library.' > "$OMNI_LIBRARY/raw/hello.md"
"$OMNI_BIN" --root "$OMNI_LIBRARY" search knowledge --mode bm25 --tag research --canonical --json
"$OMNI_BIN" --root "$OMNI_LIBRARY" search Hello --mode literal --tag research --canonical --json
"$OMNI_BIN" --root "$OMNI_LIBRARY" add /absolute/path/to/document.docx
"$OMNI_BIN" --root "$OMNI_LIBRARY" search "document phrase" --mode literal --canonical --json
```

Foreground search refreshes the native corpus from current files.
Canonical JSON includes library identity, coverage, and source evidence; omit `--canonical` for the retained result-array format.
Repeated `--tag` filters require every complete tag, ignoring case unless `--case-sensitive` is set.
The examples use Markdown front-matter tags; legacy import tags have the open bridge limitation listed below.
Use `--mode regex` (or `grep`) for regular expressions and `--mode files` for metadata discovery.
The installed parser succeeded on a small DEFLATE DOCX, refused a new source while its sibling was absent, and succeeded after restoration.
Do not remove the parser for normal use.

## Connect An MCP Client

Launch the canonical stdio server:

```sh
"$OMNI_BIN" --root "$OMNI_LIBRARY" serve --stdio
```

Give each client the absolute executable path and arguments `["--root", "/absolute/path/to/library", "serve", "--stdio"]`.
This server uses MCP 2025-11-25 and defaults to read-only access.
For existing integrations, `serve-mcp` retains the seven-tool MCP 2024-11-05 interface.
See [agent operations](docs/agents.md) for that compatibility interface.
MCP search returns evidence; it does not generate an answer or validate a source's claims.

## Optional WeMM Setup

Model processing is off by default.
Normal native search does not load weights or contact an embedding provider, even if initialization saved a provider configuration.
Real WeMM weights and quality are not qualified in this version.

When the engineer supplies an approved package and provider:

1. Verify the package and manifest against the approved identity, then start the provider with that package's launch instructions.
2. Set `embedding` in the library's `omni-knowledge.json` to its exact `endpoint`, `model`, `dimension`, `package_fingerprint`, and `embedding_space`; set `timeout_seconds` from 1 to 600.
Use only a loopback HTTP endpoint; do not copy identity values from a different package.
3. Check the configured provider and explicitly build the compatibility vector index:

```sh
"$OMNI_BIN" --root "$OMNI_LIBRARY" doctor --embeddings --json
"$OMNI_BIN" --root "$OMNI_LIBRARY" index --embeddings
```

4. Complete engineer item N03 before expecting semantic queries through the native CLI/MCP.
This artifact has no model-selection flag: those queries return `provider_unavailable`, including after the commands above.
N03 must bind the approved package and manifest through `NativeLibraryService::select_embedding`, then check actual loaded weights and the approved quality cases.
The [multimodal guide](docs/multimodal.md) describes provider identities, requests, and ASR; its historical package measurements do not qualify a new package.

## Build And Install On An Apple Silicon Mac

These steps are for the engineer's acceptance; this version makes no Mac qualification claim.
Use a clean checkout of this version on native `arm64` macOS.
Install Apple command-line tools, Rust 1.99.0 with Cargo, Python 3.11 or newer, and system `ffmpeg`/`ffprobe`.
Run from the repository root, using new stage and installation directories:

```sh
set -e
test "$(uname -s)" = Darwin
test "$(uname -m)" = arm64
export CARGO_TARGET_DIR="$PWD/temp/mac-target"
export CARGO_BUILD_JOBS=2
export LINDERA_BUILD_DICTIONARY_CACHE_DIR="$PWD/temp/mac-dictionary"
python3 tools/kernel-probe/prepare-dictionary.py --cache "$LINDERA_BUILD_DICTIONARY_CACHE_DIR"
cargo build --workspace --release --bins --no-default-features --locked
python3 -B scripts/bundle-headless.py --target aarch64-apple-darwin --stage "$PWD/dist/macos-arm64"
python3 -B scripts/bundle-headless.py --target aarch64-apple-darwin --stage "$PWD/dist/macos-arm64" --verify
mkdir -p "$HOME/.local/opt"
OMNI_MAC_INSTALL="$HOME/.local/opt/omni-knowledge-dfa0902"
mkdir "$OMNI_MAC_INSTALL"
cp -R "$PWD/dist/macos-arm64/." "$OMNI_MAC_INSTALL/"
OMNI_BIN="$OMNI_MAC_INSTALL/omni"
"$OMNI_BIN" --help
```

The initial checks stop on another OS or an x86_64/Rosetta shell.
The stage must contain distinct sibling executables `omni`, `parser-worker`, and `omni-fsevents`.
Cargo can fetch locked registry dependencies; the verified dictionary cache is only a build prerequisite.
See [toolchain setup](docs/toolchain.md) and [packaging](scripts/README.md#headless-product-archive) for details.

Mac acceptance checklist:

- Repeat the new-library, BM25, literal, and DOCX checks above with the installed Mac executable.
- Run `cargo test -p knowledge-local --test fsevents_native --no-default-features --locked -- --test-threads=1` for real FSEvents callbacks, root drift, deadlines, and helper collection.
Require the actual native test to pass; a zero-test Linux run is not Mac evidence.
- Connect both Claude and Codex clients to that installed executable with `serve --stdio`; check discovery, search evidence, errors, and the default read-only boundary.
- If choosing automatic speech recognition (ASR), follow the [pinned setup and real-model opt-in](docs/multimodal.md#asr-setup-and-evidence), including `actual_local_asr_recognizes_known_speech`.
Record it as unrun if not chosen; ordinary tests do not qualify real speech recognition.

## Known Limits And Open Items

- Native foreground refresh is bounded to 1,000 sources and 64 MiB of original input per invocation, with a 512 MiB memory ceiling.
Text extraction uses an independent 8 MiB input/decoded limit; inspect coverage for failed, unavailable, or metadata-only sources.
- There is no persistent CLI watch daemon; the native FSEvents API still needs Mac acceptance.
- Legacy `add --tag` metadata was not reflected in native Query filtering in the example probe.
Use Markdown front-matter tags for the demonstrated filter workflow; the import-tag bridge remains open.
- N03 remains the engineer's optional model selection/real-weight/quality item; N04 remains native Mac, client, and optional ASR acceptance.
Neither blocks this qualified Linux first usable version.
- GUI milestone M7, Linux ARM64, physical resource qualification, and complete implementation of every catalogued command are outside this qualification.
- Dependency notice gaps remain recorded in the archive; public distribution, notarization, and compliance certification are not claimed.

Back up originals, `omni-knowledge.json`, and `.omni/metadata/`; generated indexes can be rebuilt.
Read [storage and recovery](docs/specs/storage.md) before migration, [the desktop guide](docs/gui.md) for the retained GUI, and [the documentation map](docs/README.md) for contracts.
Contributors start with [AGENTS.md](AGENTS.md) and [CONTRIBUTING.md](CONTRIBUTING.md).
Original code uses [Apache-2.0](LICENSE); third-party components and [foundational source articles](assets/foundations/README.md) retain their licenses and provenance.
See the [FFmpeg packaging boundary](docs/dependencies.md#ffmpeg-packaging-boundary) before distribution.
