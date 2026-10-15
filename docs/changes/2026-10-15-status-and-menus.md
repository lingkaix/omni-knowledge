# Status Hints And Native Preview Menus

Source: The engineer reported covered hints and file-menu focus problems, then requested one normal status row with grouped library identity and direct index actions.
The [product requirements](../specs/product.md) own the requested behavior; the [native preview decision](../decisions/003-native-preview.md) owns the AppKit boundary.

## Result

Version 0.1.0, build 9, normally uses one 30-pixel status row.
The tree no longer reserves separate instruction and destination strips.
Routine preview-ready and file-row instruction messages are omitted; the transfer destination stays in the status row.
The configured display name and opened root path share one Finder target.
Index state and the real index-refresh button occupy the right end, separate from library identity and dependency guidance.
Clicking stale or missing-index status directly indexes the confirmed opened root.
A different, unopened Library path directs the user to Open or Initialize and cannot index the previous root.
Coverage warnings remain visible for their root after successful publication and later preview selection.
Full hints temporarily wrap above the fixed row while hovered, inside bounds that the native preview cannot cover.
Stable control identities clear hints after dynamic status changes and reject another control's obsolete exit event.
Right-click selects the exact source or search occurrence and can replace another file's open menu directly.
Preview interaction dismisses that menu through native view focus without consuming the preview action.
Menu session identities reject delayed dismissals from an older target.
No storage format, resource permission, document-script privilege, or automatic indexing behavior changed.

## Verification

Checks ran on Apple M5 Pro, macOS 27.0.1, Rust/Cargo 1.99.0.

| Command or observation | Result |
| --- | --- |
| `cargo test --offline --release --features gui --lib gui::tests` | 29 passed; 0 failed; 0 ignored; 21 filtered out |
| `cargo clippy --offline --release --all-targets --features gui -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `git diff --check` | Passed |
| `sh scripts/bundle-macos.sh` | Built and strictly verified build 9; retained the previous app |
| Fresh native launch | Opened the signed app through LaunchServices |
| Normal browsing | One row, grouped configured name and path, inline transfer target, index controls at the right |
| Hover exit into WebKit | Full hint closed without requiring a preview click |
| Dynamic index-status hint | Returned to one row after stale-to-current and missing-index-to-warning transitions |
| Stale and missing-index clicks | Published the corresponding temporary library's index |
| Refresh of a current index | Published a new generation, rather than merely checking status |
| Unopened and uninitialized paths | Opened location controls only; existing pointers and source file remained unchanged |
| Malformed DOCX coverage | Published metadata coverage, retained the warning after preview selection, and exposed its full first diagnostic on hover |
| Configured-name click | Finder opened the actual current root, even with another location entered |

Build 8 native checks established exact right-click selection, menu replacement, Escape, and dismissal in Markdown, image, PDF, audio, and video views.
This follow-up retains that menu and native-focus implementation.
Independent review accepted the final footer action guards, warning feedback, hover ownership, and governing documentation after the reported findings were corrected.
The focused checks do not repeat the earlier full-suite or Chinese composition verification.

## Boundaries

Native checks used temporary libraries; the app returned to the existing photo library without saving metadata or rebuilding its index.
Its published pointer remained unchanged across restoration, and its previous embedding toggle and viewed source were restored.
The test Finder window closed; the existing model service remained running.
Prior bundles, test pointer backups, and disposable fixtures remain outside tracked product files for recovery and investigation.
No push, public release, Applications installation, or deployment was performed.

An unrelated metadata feedback issue remains: undoing a draft to its saved value can leave the previous unsaved-status label visible.
The GUI editor owns this display-only finding; review the change-event feedback when the metadata editor is next changed, and remove this note after a regression proves the label clears.
