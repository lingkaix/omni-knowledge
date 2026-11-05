# Preserve canonical test records and compile-only guide identity

The first package-integration gate ran every Rust target successfully, then correctly refused inconsistent accounting: a ZIP short-write diagnostic without a trailing newline prefixed one libtest record, while rustdoc appended ` - compile` to the compiled guide's inventory name. The complete failing gate remains retained; neither test is excluded or invented as passing.

Capture Cargo test stderr in a separate `tests.stderr.log`, recorded alongside its stdout and command receipt. This preserves the original diagnostics without mixing them into canonical stdout records. Group/deadline ownership is unchanged. Dropping this separation reproduces the lost-record problem; the parser deliberately still rejects a prefixed record rather than accepting arbitrary substrings.

Normalize only rustdoc's precise `*.rs - (line N) - compile` outcome label to its inventory identity. The compiled guide remains a compilation observation, not a runtime product test. Duplicate runtime/compile records, failed compile observations, missing records, undeclared ignores and altered ordinary test names still fail accounting.

The instrument self-test covers real separate-channel output, diagnostic retention, compiled-guide matching and the negative controls above, in addition to existing PASS/FAIL/TIMEOUT checks. These small instrument inputs do not constitute filesystem, processor, product-bound or native-platform acceptance. The full actual workspace gate must pass again before publication.
