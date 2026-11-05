# Frozen Product Boundaries

`main.rs` and `fixture.rs` are the public, independent B1/B2 fixture from committed `8532a19c40824ca7d98c030f88cbc138bda4c7bb`.
Their blobs are respectively `8ff0f5dbd8521f2632b0fbc49ce388188f3775a7` and `1517fbbbe356e2a53847194033e9d99029eeddc0`.
No assertion, scenario, ceiling, or fixture body is changed in this repair.
The test admits its process observer with deliberate stand-ins before making actual product observations; stand-in outcomes are not product proof.

Build the real `omni` and `knowledge-processing` parser-worker binaries first with the same private target, offline dictionary supply and two-job settings.
Set `KNOWLEDGE_PROCESSING_PARSER_WORKER` to that exact parser-worker path, then run `cargo test --locked --offline -p omni-knowledge --test product_boundaries -- --test-threads=1 --nocapture`.
Use an invocation-private disk-backed TMPDIR.
The test retains unresolved fixture evidence; do not delete it to manufacture a clean result.

The working-allocation oracle remains 4,275,817 bytes for the frozen amplified workbook, and the RSS ceiling remains 536,870,912 bytes.
Ordinary parsing must produce real ordered content, not metadata-only success or an unstarted-account refusal.
The [repair record](../../docs/changes/2026-11-05-foreground-native-extraction.md) records the actual result and remaining producer boundary.
