# Desktop Workspace And Setup

Source: The engineer's desktop follow-ups on 2026-10-14 and 2026-10-15, including the reported signing failure, input problems, blocked previews, metadata saving, and stale status.
The accepted [product requirements](../specs/product.md) own behavior; [storage](../specs/storage.md) and [native preview](../decisions/003-native-preview.md) own the representation and implementation decisions.

## Result

The local artifact is version 0.1.0, build 7, at `dist/Omni Knowledge.app` with both GUI and CLI executables.
Fresh bundle assembly, nested signing, and strict verification correct the reported invalid-signature launch failure and preserve the previous app.
The app uses the supplied green mark, an optically centered icon, an offline Chinese guide, and explicit Homebrew setup for external media tools.
Compact GPUI Kit controls, aligned 28-pixel inputs, a collapsible live file tree, clearable search, contextual metadata, and a bottom status bar preserve reading space.
Native inputs restore Cocoa focus after preview interaction and support the maintained component's editing and input-method behavior.
Contained file copy, paste, drag, Finder, default-app, Quick Look, metadata, and confirmed recoverable Trash actions are available.
Upgraded indexes remain inside their knowledge folder and retain identity after a whole-folder move.
Safe source links and explicit unsupported-format messages preserve access to originals.

Preview, search, metadata saving, and explicit indexing run independently.
Selected-file facts no longer load the complete vector manifest or hash the source.
WebKit resource reads run in the background, native geometry updates are cached, and unchanged directory observations do not rebuild the tree.
Each opened library has a scoped resource origin, so delayed requests cannot read the next library's same-named source.
Metadata saving writes only the authored sidecar, preserves newer drafts, reports success or failure, and leaves indexing explicit.
Status checks are coalesced independently and refresh after publication or metadata changes.
Sidebar hints stay inside the sidebar beside the native preview.

## Verification

Checks ran on Apple M5 Pro, macOS 27.0.1, Rust/Cargo 1.99.0.

| Command or observation | Result |
| --- | --- |
| `cargo test --offline --release --features gui` | 132 passed; one real-ASR test ignored by default |
| `cargo clippy --offline --release --all-targets --features gui -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| Staged whitespace check | Authored text passed; verbatim upstream files retain the warnings described below |
| `sh scripts/bundle-macos.sh` | Built and strictly verified build 7; retained the previous bundle |
| Fresh LaunchServices launch | Native window opened successfully |
| Bundle seal fault injection | Untouched copy passed; a modified bundled license failed verification |
| Publication interruption and verification-failure probes | Prior bundle restored; failing control detected |
| License-collector boundary probes | Exact notices, source archives, scope, containment, and failure preservation passed |

The full staged `git diff --cached --check` reports only upstream whitespace in the canonical Apache license texts, the original OKF snapshot, and Mermaid's bundled source.
These files retain their original bytes for provenance and license preservation; the same check with those four paths excluded passes for authored text.

Native build-7 checks held a synthetic embedding request pending while switching document previews, entering text, deleting a word with Option, and expanding the tree.
The pending request remained unfinished throughout those actions and completed only after explicit release; this proves task independence, not model quality or a corpus performance benchmark.
Native metadata saving persisted the exact tags and description without changing the published pointer.
The footer changed from current to stale after that save and returned to current after explicit indexing.
Sidebar action hints remained fully visible beside WebKit, and root, search, tag, and metadata controls had aligned visible heights.
The actual photo library opened read-only with current status; its JPG rendered and its RAF showed the explicit native-viewer fallback message.
Earlier native checks covered file clipboard copy, paste, internal drag, Trash confirmation and recovery, contained links and anchors, and default-app launch acceptance.
The engineer confirmed Chinese composition and standard editing shortcuts after the native input correction; automation alone did not establish composition behavior.
Independent source and regression review found no remaining actionable issue in the final file-information, task-ordering, metadata, resource-origin, status, and documentation scopes.

## Boundaries And Cleanup

The app links only macOS system libraries and includes third-party notices plus unmodified MPL source archives; the inventory records notice gaps and is not a public-release compliance certification.
FFmpeg, whisper.cpp, the WeMM provider, and model weights remain separate explicit setup.
The synthetic test provider stopped cleanly; test-only Trash content was restored to its temporary fixture, and the real model service was preserved.
The running app was returned to the real photo library without saving or indexing it; disposable libraries and raw evidence remain outside tracked product files.
M3 execution, Developer ID/notarization, OCR, and full Office/EPUB layout remain outside this verification.
No push, release, installation into Applications, deployment, or publication was performed.
