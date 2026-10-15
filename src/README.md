# Application Source

Read [product requirements](../docs/specs/product.md) before changing behavior.
The library provides knowledge-base lifecycle and shared search for the CLI and GPUI application.
Extraction, EPUB packaging, embedding requests, and media segmentation each have a separate module.
`runtime.rs` owns tool discovery and allowlisted Homebrew installation; document preview cannot invoke these operations.
`status.rs` owns nonmutating folder, freshness, and local-service checks.
`file_actions.rs` owns contained desktop transfers, native file clipboard actions, Finder navigation, and recoverable Trash operations.
`file_info.rs` reads live selected-file attributes and authored metadata independently of derived search state.
`gui_assets.rs` serves the embedded native-icon inventory without runtime filesystem or network access.

Run `cargo test` for the core and `cargo build --features gui --bin omni-knowledge-gui` for the macOS interface.
