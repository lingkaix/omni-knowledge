# Native cache and history fixture portability

Run 41959300513 macOS reached actual tests: cache's 16+3 cases could not start the external Linux `flock` executable, and all 13 history cases failed their shared `History::init` fixture with `Invalid`.
Use the existing Rustix native file lock around the real Cargo consumer build; retain serialization and all source/child evidence checks, with no external utility or dependency change.
Create history fixtures below the canonical native temporary parent so Darwin's temporary-directory aliases satisfy the existing strict canonical-path contract. Do not weaken that production contract or alter any Git assertion.
Linux focused Core/history suites, workspace format, and strict all-target Clippy passed; raw macOS failures and local command/ownership receipts are retained in `lead-artifacts/trunk-integration-0105`.
This is a setup-only fix-forward, not a new Git safety review, native macOS success, resource enforcement, or final acceptance claim; actual macOS rerun remains required.
