# Application Source

Read [product requirements](../docs/specs/product.md) before changing behavior.
The library provides knowledge-base lifecycle and shared search for the CLI and GPUI application.
`storage.rs` owns buffered manifest loading, validated resident search snapshots, and per-query publication leases.
`lib.rs::admitted` routes real lexical/grep publication and search through `knowledge-retrieval` and the one Core account. Bounded deterministic legacy-snapshot shards use file-deduplicated rank fusion, not raw cross-shard BM25 scores. The retained `search.rs` module supplies metadata/vector compatibility only; its lexical and trigram builders are no longer called.
Extraction, EPUB packaging, embedding requests, and media segmentation each have a separate module.
`runtime.rs` owns tool discovery and allowlisted Homebrew installation; document preview cannot invoke these operations.
`status.rs` owns nonmutating folder, freshness, and local-service checks.
`file_actions.rs` owns contained desktop transfers, native file clipboard actions, Finder navigation, and recoverable Trash operations.
`file_info.rs` reads live selected-file dates, attributes, and the authored record's location and values independently of derived search state.
`gui.rs` groups enabled retrieval methods, contextual file details, library setup, and index actions by their user task.
It preserves ranked files with type-appropriate line and time locations, distinct-file pagination, and exact text-line preview selection.
The public inline `extract`, `epub`, and `media` modules bind mature `knowledge-processing` IO to the actual Core account. Root index outputs are handed into the retained snapshot lifetime by admitted deep copy: new buffers are reserved before copying; old buffers die before their original holdings finish. The snapshot's allocation guard is its last field, so records/vectors die first. Raw extraction/EPUB/preview compatibility returns without an owning scope prepay a deep copy before destroying the original M3 buffers and finishing their larger output envelope; only the copied-buffer credit remains until process exit.
Production owners must use the lifetime-bearing funded APIs instead of treating that compatibility limit as a native job owner.
`gui_assets.rs` serves the embedded native-icon inventory without runtime filesystem or network access.

`native_service::NativeProcessing::{extract,export_epub,segment_video}` owns the selected descriptor/current-policy carrier, actual Claim, complete paid value and staging namespace through last-reader destruction and controlled shutdown.
These methods use the committed M3 funded descriptor/held-FD APIs and return the SAME source owner for delivery; see the [native consumer guide](native_service/README.md).
Held EPUB members retain actual current-version pins in that final delivery scope, including members admitted through separate native scopes.
Root unresolved processing errors now own their actual leases in a Box, not a leaked global; controlled drop retries bounded native collection and never treats unknown generated-write deltas as zero.
The [repair record](../docs/changes/2026-11-05-foreground-native-extraction.md) tracks remaining generated-write reconciliation and caller ports.

`lib.rs::native_service::NativeLibraryService` composes the real selected-root browse/version/range/query/exposure host with `omni_service::runtime`. Owner and selection capabilities are separate from transport facts and decoded requests. Other owners attach typed `DomainHost` implementations; absent mount/history/mutation/package adapters refuse honestly. See [native runtime](../crates/omni-service/NATIVE.md).

Build `parser-worker` from `knowledge-processing` before running opaque extraction. Installed binaries require that real helper beside the executable; tests with a private target set `KNOWLEDGE_PROCESSING_PARSER_WORKER` to its exact built path. Run the headless suites with `--test-threads=1`: otherwise unrelated opaque-fixture tests can legitimately compete for M3's full-available-memory child promise and return typed waiting-memory. Dedicated concurrency tests still run their real concurrent work. Preserve those contention receipts, do not treat them as successful jobs.

Run `cargo build --features gui --bin omni-knowledge-gui` for the optional macOS interface; GUI activation and final native platform acceptance remain separate.
