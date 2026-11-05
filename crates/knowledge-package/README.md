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
`Work` borrows one `knowledge_core::admission::Holding` from `Account::current`. `Holding::grow` runs before each retained buffer or generated write. Scratch that dies inside the call uses a second holding on that same account and `finish`es when the buffer drops. The caller's holding is not finished here. See [OWNER.md](OWNER.md).

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
| `Holding::grow` / `written` | Charge the one process account before retained buffers and generated writes. | A private counter could admit work the account refused. |
| Scratch `finish` | Release a dead inflate window without releasing retained names or output. | Every header would stay charged until the host finished the whole call. |
| Private generation plus owner traits | Keep a previous complete generation and let storage and history perform the final write. | This crate would grow a second rollback and Git engine. |

No registry, workflow engine, or destination writer is added in this slice.
SSH signer policy, kernel search of STORED generations and float32 vectors, genuine v1 mapping, history bundle verification, M1 install, and EPUB presets remain required and unfinished. `Signature::Unverified` is not trust. `required_features` still refuse installation. See [OWNER.md](OWNER.md).

## Checks

```sh
cargo check -p knowledge-package --locked --offline
cargo clippy -p knowledge-package --all-targets --locked --offline -- -D warnings
cargo test -p knowledge-package --locked --offline
cargo doc -p knowledge-package --no-deps --locked --offline
```
