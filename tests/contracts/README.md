# Independent Public Contract Checks

Read [the test strategy](../../docs/specs/test-strategy.md) and [the disposition record](../../docs/changes/2026-11-04-test-disposition.md) first.
`tests/preserved_behavior.rs` includes this directory through Rust modules, so Cargo executes these checks without another runner.

`writer_release.rs` exercises a real index operation and subsequent metadata mutation while an unrelated child waits before exec.
It controls only the external provider and child scheduling, not the library's lock implementation.
The check fails on the OLD snapshot and is an existing-defect regression, not a passing OLD/NEW preservation replacement.
It requires Unix process and socket APIs; the child uses only async-signal-safe reads and writes before exec.
The fixture releases and reaps the child before its deciding assertions.

Run `cargo test --locked --test preserved_behavior` for the independent module checks.
No existing regression is retired by these additions.
