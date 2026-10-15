# Search And File Details Usability

Source: The engineer requested a coherent interface for nontechnical users, with contextual search options, clear file properties, and understandable metadata persistence.
The [product requirements](../specs/product.md) own behavior; the [native controls decision](../decisions/003-native-preview.md) records the presentation and worker boundaries.

## Result

Version 0.1.0, build 10, uses one primary search row.
Search options and the trailing clear button belong inside the search field.
Search mode and exact tags appear only when options are expanded; the summary identifies applied restrictions and unapplied edits.
A successful search owns its displayed conditions; a failed or obsolete request cannot relabel earlier results.
Clear cancels pending searches, clears text and tags, and restores the live directory tree.
Adding files belongs beside the tree, library setup and next-refresh semantic generation belong in Library, and index refresh remains in the footer.
The semantic checkbox neither starts a model nor persists a new library preference.

File details separates copyable source paths and readable properties from labelled Tags and multiline Notes.
Dates use the observed local timezone, sizes retain exact bytes, and unavailable dates remain explicit.
Saved values live in the existing path-keyed JSON record, remain visible after reopening, and can be revealed in Finder.
Saving updates that record without changing the original or publishing an index; the interface explains the separate refresh step.
Undo back to saved values clears the unsaved indicator.
Finder dispatch revalidates source ownership and the saved record, then uses the existing ten-second launcher bound.
Compact status labels stay on one line while hover details retain complete diagnostics.
Storage schemas, preview permissions, and retrieval algorithms are unchanged.

## Verification

Checks ran from the repository root on Apple M5 Pro, macOS 27.0.1, with Rust/Cargo 1.99.0.

| Command or observation | Result |
| --- | --- |
| `cargo test --offline --release --features gui --lib` | 61 passed; 0 failed; 0 ignored |
| `cargo test --offline --release --test file_info` | 5 passed; 0 failed; 0 ignored |
| `cargo clippy --offline --release --all-targets --features gui -- -D warnings` | Passed |
| `cargo fmt --all -- --check`, `git diff --check`, and local Markdown links | Passed |
| `sh scripts/bundle-macos.sh` | Built build 10, verified its ad-hoc signature, and preserved the previous bundle |
| Fresh signed-app launch | Opened through LaunchServices with a temporary library |
| Search hierarchy | One normal search row; mode and tags in Search options; Add files beside the tree; semantic checkbox in Library |
| Path copying | Absolute and relative paths preserved Chinese characters and spaces when pasted into the search field |
| File details | Familiar format name, readable size with exact bytes, local Created and Modified dates, labelled Tags and two-line Notes |
| Save and undo | Unsaved feedback cleared on undo; Unicode tags and multiline notes persisted to the actual JSON record |
| Reopening | A fresh app launch reloaded the saved tags and notes |
| Persistence boundary | Source content and published index pointer remained unchanged through save, inspection, copying, and option changes |
| Saved-record reveal | Finder selected the exact JSON sidecar; the test Finder window closed afterwards |
| Explicit refresh and tags | Footer refresh published a new temporary generation; the saved Chinese tag retrieved its file |
| Unapplied and failed searches | Erasing a draft tag retained the applied tag summary; an invalid text pattern retained the previous successful result and conditions |
| Clear | The inline button restored the complete live tree from a tag-only filter and failed-search draft |
| Final error-row check | An invalid text pattern displayed one readable status line; hover retained the complete multiline diagnostic above the native preview |

Independent review closed the applied-filter identity and bounded Finder-dispatch findings.
Unit coverage includes missing dates, sidecar identity and containment, readable labels, dirty-state reversal, and multiline diagnostic presentation.
Native Chinese text entry here used paste; this check does not repeat the earlier human verification of input-method composition.

## Boundaries

Test metadata and index writes were confined to a disposable knowledge library.
The existing photo library's source, authored metadata, and index pointer remained unchanged.
The app returned to that library, its previously viewed source and file details, with the earlier semantic-generation option restored.
Temporary fixtures, raw evidence, and previous signed bundles remain outside tracked product files.
Clipboard tests leave a temporary test-file path on the clipboard.
Unsaved drafts are not retained when selecting another file; the desktop guide states that users must save before navigation.
No tool installation, model download, remote publication, push, or Applications installation occurred.
