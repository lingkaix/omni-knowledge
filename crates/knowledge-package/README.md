# Knowledge Package

This crate owns portable `omni.knowledge-package` version 1 ZIP capture, hostile framing checks, preview, and private restore staging.
[Package](../../docs/specs/package.md) owns the format. Storage owns final no-clobber apply. History owns Git transactions. Processing owns EPUB.
The public functions below are the frozen integration surface for later CLI and MCP wiring.

## Public Surface

`inspect` reads one archive, rejects hostile framing before payload materialization, and hashes every inventoried payload.
`capture` streams caller-supplied bytes into a private archive, validates that archive, and hard-links it only when the destination name is absent.
`stage` copies verified payloads into a new private generation. An existing `INCOMPLETE` generation is returned as interrupted and is not rewritten.
`apply` calls `SafeApply::apply_text_change` or `install_new` and `GitTransactions::admit_inert_bundle`, passing a mutable `VerifiedSource` and the same `Work` to each owner.
It does not write the destination and it does not run Git.
This is a breaking Rust interface correction: `apply(staged, storage, history, work)` requires admission for recovery and owner copies; the owner traits accept a source capability, not a path.
`v1_import_plan` hashes a foreign directory and returns `CompatibilityRefused`. Host context does not fabricate owner, revision, or review facts.
`Work` borrows one `knowledge_core::admission::Holding` from `Account::current`. `Holding::grow` runs before each retained buffer or generated write. Scratch that dies inside the call uses a second holding on that same account and `finish`es when the buffer drops. The caller's holding is not finished here. See [OWNER.md](OWNER.md).

Indexes, history bundles, and journals stay inside the private generation. Core and original paths are the only home candidates. Conflicts are not sent to storage.
Private and destination parent directories must be absolute native Unix paths; every ancestor is opened with `NOFOLLOW` from a held directory descriptor.
Returned `Staged` paths are display information, not source authority.
A private authority retains the generation descriptor, operation, snapshot, and conflict facts.
Recovery hashes fresh `MANIFEST` bytes and compares `PLAN` incrementally against that manifest and operation, with an exact EOF check.
An old generation without this binding is refused and preserved, not migrated or deleted.

## Bounds

Callers pass Core `Budget` and explicit format quotas. Zero closes the call.
Declared uncompressed sizes are not allocation permission. Deflate output is counted from bytes actually produced.
A single request larger than the whole call budget is `TooLarge`. Current exhaustion is `Waiting`.
This crate adds no corpus cap and no second reservation counter.
A lexical preflight counts JSON nodes and string bytes before serde/JCS allocation, including optional arrays and objects.
Representation envelopes include container capacity, map edges, semantic clones, canonical output, and key-sorting workspace; they are deliberately conservative, not allocator measurements.
Scratch stays live until its buffers drop; returned semantic data stays on the caller holding until the host settles it.
Capture uses one source window and a real native `ZipWriter`, then writes the final manifest.
Its `Write + Seek` gate admits new archive extent before growth, including local headers, compression expansion, central records, and ZIP64; overwritten headers do not consume a second extent.
Each actual short write reconciles new written bytes with the same account.
A failed partial archive remains private evidence.
Manifest size follows the caller quotas and shared admission, not the previous unowned 1 MiB parse ceiling.
No source-payload RAM ceiling or record-count workaround was added.

## Compiled Owner Handoff

The following example is compiled by the crate's documentation test.
An owner must call `copy_to` into its private candidate and publish only after it returns success and the owner's normal destination/version/no-clobber checks succeed.
`copy_to` hashes the exact copied stream and checks the held descriptor's identity, size, modification time, and change time; it never reopens a mutable source path.
A failure can leave private partial output but is not permission to publish.

```no_run
use knowledge_package::{apply, Applied, Bounds, Error, GitTransactions, SafeApply,
    Staged, VerifiedSource, Work};
use knowledge_core::admission::Holding;
use std::{fs::File, sync::atomic::AtomicBool};

fn prepare_private_candidate(
    source: &mut VerifiedSource,
    candidate: &mut File,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    source.copy_to(candidate, work)?;
    candidate.sync_all().map_err(|_| Error::Io)
    // Storage still owns atomic publication and destination conflict checks.
}

fn restore(
    staged: &Staged,
    storage: &mut dyn SafeApply,
    history: &mut dyn GitTransactions,
    bounds: &Bounds,
    holding: &mut Holding,
    cancel: &AtomicBool,
) -> Result<Applied, Error> {
    let mut work = Work::new(bounds, holding, cancel)?;
    apply(staged, storage, history, &mut work)
}
```

## Necessity And Ablation

| Mechanism | Present need | What removal loses |
| --- | --- | --- |
| Owned framing reader | Reject traversal, links, encryption, overlap, and ZIP64 disagreement before copy. | The mature ZIP writer would still accept hostile input through a permissive extract path. |
| Streaming native ZIP and extent gate | Avoid retaining payloads and a rendered archive while admitting every new native extent. | A payload-only estimate misses framing and compression growth. |
| Node/string preflight and live metadata guards | Fund real serde/JCS/semantic representations before allocation. | Many small optional elements can cost much more than their encoded bytes. |
| Held directories and `VerifiedSource` | Keep confined native source identity through owner delegation and exact-stream verification. | Hashing then passing a mutable path proves a different read. |
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
