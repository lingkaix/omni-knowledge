# Clean-checkout headless artifact staging

CI run 41959300513 Linux passed the workspace gate, then failed staging because `dist/` did not exist; local prior directories had hidden the missing-parent case.
The stage helper now creates missing parents while refusing every existing final entry. No product, dependency, notice, parser, or resource-limit rule changes.
Four real-filesystem regressions cover a fresh parent and preservation of an existing file, directory, and dangling link; both native workflow staging steps run them.
Actual Linux release staging from an absent nested parent and archive verification passed, using the already qualified unchanged Rust binaries and exact normal/build notice closure.
Removing parent creation reproduces the clean-checkout failure; reusing or deleting an existing final entry would violate the retained no-clobber checks. No full Rust-suite replay was needed for this packaging-only correction.
Raw CI failure and local staging/verification/ownership receipts remain in `lead-artifacts/trunk-integration-0105`; native macOS and final product acceptance remain separate.
