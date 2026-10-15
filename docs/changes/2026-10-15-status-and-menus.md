# Status Hints And Native Preview Menus

Source: The engineer reported covered status hints, redundant sidebar strips, incorrect right-click selection, and menus that survived preview clicks.
The [product requirements](../specs/product.md) own the requested behavior; the [native preview decision](../decisions/003-native-preview.md) owns the AppKit boundary.

## Result

Version 0.1.0, build 8, places contextual hints, operation feedback, and the transfer destination inside one bottom status bar.
The tree no longer reserves separate instruction and destination strips.
Long hints wrap within a bounded, scrollable layout above the fixed status buttons; the native preview ends above this layout.
Right-click selects the exact source or search occurrence and can replace another file's open menu directly.
Preview interaction dismisses that menu through native view focus without consuming the preview action.
Menu session identities reject delayed dismissals from an older target.
No storage format, resource permission, document-script privilege, or automatic indexing behavior changed.

## Verification

Checks ran on Apple M5 Pro, macOS 27.0.1, Rust/Cargo 1.99.0.

| Command or observation | Result |
| --- | --- |
| `cargo test --offline --release --features gui --lib gui::tests` | 26 passed; 0 failed; 0 ignored; 21 filtered out |
| `cargo clippy --offline --release --all-targets --features gui -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `git diff --check` | Passed |
| `sh scripts/bundle-macos.sh` | Built and strictly verified build 8; retained the previous app |
| Fresh native launch | Opened the signed app through LaunchServices |
| Right-click target and replacement | Highlight and preview followed each clicked file; the replacement menu stayed open |
| Preview outside click | Closed the menu in Markdown, image, embedded PDF, audio, and video views |
| Escape | Closed the visible file menu |
| Root-path and dependency hints | Fully visible inside the footer, above its status buttons and outside WebKit bounds |

Independent review found no actionable source issue in the changed menu, focus, and footer behavior.
The focused checks cover this correction; they do not repeat the preceding change's full-suite or Chinese composition verification.

## Boundaries

Native checks used a temporary library; the app was returned to the existing photo library without saving metadata or rebuilding its index.
Test media playback stopped before restoration, and the existing model service remained running.
The prior bundle and disposable fixture remain available for recovery and investigation outside tracked product files.
No push, public release, Applications installation, or deployment was performed.

An unrelated metadata feedback issue remains: undoing a draft to its saved value can leave the previous unsaved-status label visible.
The GUI editor owns this display-only finding; review the change-event feedback when the metadata editor is next changed, and remove this note after a regression proves the label clears.
