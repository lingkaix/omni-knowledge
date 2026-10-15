# First Usable Version

Source: The engineer's 2026-10-14 request for a testable local Rust and GPUI knowledge library.

## Result

Version 0.1.0 provides a GPUI desktop application, CLI, and stdio MCP tools through one Rust core.
It supports nested knowledge roots, preserved imports, general metadata, exact tags, grep, BM25, semantic/hybrid retrieval, media evidence times, document previews, and EPUB export.
Initialization includes the original LLM Wiki and OKF snapshots.
The [product](../specs/product.md), [storage](../specs/storage.md), and [search](../specs/search.md) specifications own behavior; [decisions](../decisions/README.md) retain the design reasons.

## Verification

Checks ran on Apple M5 Pro, macOS 27.0.1, Rust/Cargo 1.99.0.

| Command | Result |
| --- | --- |
| `cargo test --offline --features gui` | 45 passed; one actual-ASR test ignored by default |
| `cargo test --offline --release --features gui --test knowledge_base` | Final core regressions: 21 passed |
| `cargo test --offline --release --features gui --test mcp` | Final agent regressions: two passed |
| `cargo test --test embedding actual_local_asr_recognizes_known_speech -- --ignored` with the configured local runtime | One passed; [setup](../multimodal.md#asr-setup-and-evidence) |
| `cargo clippy --offline --release --all-targets --features gui -- -D warnings` | Passed |
| `cargo fmt --all -- --check` and `git diff --check` | Passed |

Together, these runs exercised 50 distinct passing tests; they were not one run with the opt-in test enabled.
Actual WeMM text/image queries returned the expected image and video evidence at 0–10 seconds.
Actual whisper.cpp recognition and audio retrieval returned the original recording with source times.
Native window checks covered Chinese input, Markdown/Mermaid/highlighting, PDF, Office, EPUB, video playback, timestamped audio, and metadata edits.
Independent review findings were resolved with focused checks; final reviewed scopes had no actionable finding.

The original window check did not verify a fresh launch of the final assembled bundle.
A later user report exposed its missing bundle signature; the corrected packaging result is recorded in the [desktop workspace change](2026-10-15-desktop-workspace.md).

## Limits And Artifacts

The app is `dist/Omni Knowledge.app`; the demo is `temp/demo-knowledge/`.
OCR, full Office/EPUB layout, automatic filesystem watching, large-corpus benchmarking, and M3 runtime verification remain incomplete.
Image queries and EPUB export use the CLI; text extraction is bounded to 8 MiB per source.
Model/tool setup is described in [dependencies](../dependencies.md) and [the multimodal guide](../multimodal.md).
No commit, push, installation, deployment, or publication was performed for this initial delivery.
