# Knowledge Package

This crate owns portable `omni.knowledge-package` version 1 ZIP capture, hostile framing checks, preview, and private restore staging.
[Package](../../docs/specs/package.md) owns the format. Storage owns final no-clobber apply. History owns Git transactions. Processing owns EPUB.
The public functions below are the frozen integration surface for later CLI and MCP wiring.

## Public Surface

`inspect` reads one archive, rejects hostile framing before payload materialization, and hashes every inventoried payload.
`capture` streams caller-supplied bytes into a private archive, validates that archive, and hard-links it only when the destination name is absent.
`stage` copies verified payloads into a new private generation. An existing `INCOMPLETE` generation is returned as interrupted and is not rewritten.
`apply` calls `SafeApply::apply_text_change` or `install_new` and `GitTransactions::admit_inert_bundle`. It does not write the destination and it does not run Git.
`v1_import_plan` hashes a foreign directory and returns `CompatibilityRefused`. Host context does not fabricate owner, revision, or review facts.
`Expansion::grow` and `written` are the only memory and generated-storage admission calls. Map them to `knowledge_core::admission::Holding` when that type is linked. Do not finish the holding inside the call.

Indexes, history bundles, and journals stay inside the private generation. Core and original paths are the only home candidates. Conflicts are not sent to storage.

## Bounds

Callers pass Core `Budget` and explicit format quotas. Zero closes the call.
Declared uncompressed sizes are not allocation permission. Deflate output is counted from bytes actually produced.
A single request larger than the whole call budget is `TooLarge`. Current exhaustion is `Waiting`.
This crate adds no corpus cap and no second reservation counter.

## Necessity And Ablation

| Mechanism | Present need | What removal loses |
| --- | --- | --- |
| Owned framing reader | Reject traversal, links, encryption, overlap, and ZIP64 disagreement before copy. | The mature ZIP writer would still accept hostile input through a permissive extract path. |
| Actual expand counter | Stop bombs when output exceeds the caller quota or ratio. | Declared sizes would choose the buffer. |
| `Expansion` calls | Use the one process account for every buffer and generated write. | A private pool could admit work the account refused. |
| Private generation plus owner traits | Keep a previous complete generation and let storage and history perform the final write. | This crate would grow a second rollback and Git engine. |

No registry, workflow engine, or destination writer is required for this slice.
Direct index search and SSH signature trust stay outside this crate. `Signature::Unverified` is not a valid signature.

## Checks

```sh
cargo check -p knowledge-package --locked --offline
cargo clippy -p knowledge-package --all-targets --locked --offline -- -D warnings
cargo test -p knowledge-package --locked --offline
cargo doc -p knowledge-package --no-deps --locked --offline
```
