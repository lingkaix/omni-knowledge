# Acceptance And Regression Tests

Read [product requirements](../docs/specs/product.md) and [verification](../docs/verification.md) before adding checks.
Tests exercise public library behavior and the real CLI/protocol boundary.

- `knowledge_base.rs` covers preservation, path ownership, arbitrary binary metadata, Chinese search, reconciliation, transcripts, and safe generation retirement.
- `extraction.rs` covers native PDF and Office text, inert previews, EPUB ordering, contained export, and explicit size limits.
- `embedding.rs` covers loopback provider identities, requests, vector validation, timeouts, real ffmpeg boundaries, and local ASR integration.
- `mcp.rs` exercises the actual stdio binary and prevents malformed metadata input from changing tags.
- `runtime.rs` exercises discovery and installer outcomes with isolated executables; it does not install Homebrew packages.
- `status.rs` covers folder classification, bounded freshness checks, derived-state faults, and local service identity without sending source contents.
- `file_actions.rs` covers source containment, no-clobber transfers, metadata preservation, and mocked recoverable file operations.
- `file_info.rs` covers live selected-file facts without index reads, source parsing, or writer waits, including malformed metadata and ownership failures.
- `src/gui.rs` covers scoped preview resources, media ranges, timed evidence, coalesced read intents, metadata save/publication separation, and authored draft revisions.

Run `cargo test --features gui` with ffmpeg, ffprobe, macOS textutil, and local loopback listener permission.
Real model and native-window observations belong in the change record; simulated responses cannot prove model ranking or native rendering.
