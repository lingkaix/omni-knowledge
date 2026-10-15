# Application Source

Read [product requirements](../docs/specs/product.md) before changing behavior.
The library provides knowledge-base lifecycle and shared search for the CLI and GPUI application.
`storage.rs` owns buffered manifest loading, validated resident search snapshots, and per-query publication leases.
Extraction, EPUB packaging, embedding requests, and media segmentation each have a separate module.
`runtime.rs` owns tool discovery and allowlisted Homebrew installation; document preview cannot invoke these operations.
`status.rs` owns nonmutating folder, freshness, and local-service checks.
`file_actions.rs` owns contained desktop transfers, native file clipboard actions, Finder navigation, and recoverable Trash operations.
`file_info.rs` reads live selected-file dates, attributes, and the authored record's location and values independently of derived search state.
`gui.rs` groups search options, contextual file details, library setup, and index actions by their user task.
It preserves ranked files with expandable evidence, distinct-file pagination, and exact text-line preview selection.
`extract.rs` supplies safe rendered documents and numbered source or extracted text, with explicit stale-line and large-text notices.
`gui_assets.rs` serves the embedded native-icon inventory without runtime filesystem or network access.

Run `cargo test` for the core and `cargo build --features gui --bin omni-knowledge-gui` for the macOS interface.
