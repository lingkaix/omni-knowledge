# Omni Knowledge

Omni Knowledge keeps original files in ordinary folders and returns source-linked search results through a headless CLI and MCP.
The first usable version is qualified on Linux x86_64 from code commit `c330983`.
It covers lexical/BM25 search, Japanese BM25, DOCX extraction, and canonical and legacy MCP.
See the [qualification summary](first-usable-version.md) for the exact artifact and evidence.

## Start On Linux

Use `omni-knowledge-linux-x86_64-n07-c330983.tar.gz` supplied with this version.
Its SHA-256 is `c179f63dc1ebfc86822291a3439bc69fb780ec6b11e08240b3ffd13c4357e217`.
Keep the complete extracted directory, including `parser-worker`, licenses, and notices.

```sh
set -e
OMNI_ARCHIVE=/absolute/path/to/omni-knowledge-linux-x86_64-n07-c330983.tar.gz
printf '%s  %s\n' 'c179f63dc1ebfc86822291a3439bc69fb780ec6b11e08240b3ffd13c4357e217' "$OMNI_ARCHIVE" | sha256sum --check
mkdir -p "$HOME/.local/opt"
OMNI_INSTALL="$HOME/.local/opt/omni-knowledge-c330983"
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
"$OMNI_BIN" --root "$OMNI_LIBRARY" add /absolute/path/to/document.docx --tag research
"$OMNI_BIN" --root "$OMNI_LIBRARY" search "document phrase" --mode literal --tag research --canonical --json
```

Foreground search refreshes the native corpus from current files.
Canonical JSON includes library identity, coverage, and source evidence; omit `--canonical` for the retained result-array format.
Repeated `--tag` filters require every complete tag, ignoring case unless `--case-sensitive` is set.
Markdown front-matter tags and `add --tag` both reach these filters.
Import writes asset tags to the canonical `sources/<source-id>.md` pointer, using the actual inventory UUID; preserve that filename when editing its annotations.
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

Model processing is optional and off by default; normal native search does not load weights or contact an embedding provider.
Real WeMM weights and quality are not qualified, and this artifact has no CLI option to enable the missing native model binding.
Do not put provider settings or weights in the portable `omni-knowledge.json` manifest.

When an approved package exists, the engineer must:

1. Verify its package bytes and manifest against the approved fingerprint, model identity, dimension, and embedding space.
2. Start its provider on the selected loopback endpoint with that package's launch instructions.
3. Complete N03: select the actual opened package and manifest through `NativeLibraryService::select_embedding`, with the existing native owner, provider configuration, and prepaid Work, then connect that selection to foreground refresh/query.
Installing weights or building a compatibility index alone does not make this selection.
4. With that binding installed, run the approved real-weight/quality cases and a native semantic request:

```sh
"$OMNI_BIN" --root "$OMNI_LIBRARY" search "approved quality phrase" --mode semantic --canonical --json
```

Until N03 ships, this request returns `provider_unavailable`.
The [multimodal guide](docs/multimodal.md) describes provider identities, requests, and ASR; historical package measurements do not qualify a new package.

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
OMNI_MAC_INSTALL="$HOME/.local/opt/omni-knowledge-c330983"
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
- libraries created before 7852835 are pre-release format: re-run init and re-import.
Initialize a new empty library and re-import the originals; the old library remains untouched and readable for inventory/preview.
Version-1 native Query and mutations refuse; no migration is planned.
- New libraries use version 2 with `pages/` and canonical `sources/` pointers.
The retained `metadata` editor and generic source-details adapter are not ported; edit visible pointer annotations directly.
Legacy MCP `read` returns bounded text and the same canonical source annotations as Query, including tags and the source-pointer path.
Optional source facts remain null where Query has no admitted observation.
- N03 remains the engineer's optional model selection/real-weight/quality item; N04 remains native Mac, client, and optional ASR acceptance.
Neither blocks this qualified Linux first usable version.
- GUI milestone M7, Linux ARM64, physical resource qualification, and complete implementation of every catalogued command are outside this qualification.
- Dependency notice gaps remain recorded in the archive; public distribution, notarization, and compliance certification are not claimed.

Back up originals, `omni-knowledge.json`, `pages/`, `sources/`, and other authored files, including the journal; generated indexes can be rebuilt.
Read [storage and recovery](docs/specs/storage.md) for the format, [the desktop guide](docs/gui.md) for the retained GUI, and [the documentation map](docs/README.md) for contracts.
Contributors start with [AGENTS.md](AGENTS.md) and [CONTRIBUTING.md](CONTRIBUTING.md).
Original code uses [Apache-2.0](LICENSE); third-party components and [foundational source articles](assets/foundations/README.md) retain their licenses and provenance.
See the [FFmpeg packaging boundary](docs/dependencies.md#ffmpeg-packaging-boundary) before distribution.
