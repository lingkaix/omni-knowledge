# Knowledge Package

This crate owns portable `omni.knowledge-package` version 1 ZIP capture, hostile framing checks, preview, and private restore staging.
[Package](../../docs/specs/package.md) owns the format. Storage owns final no-clobber apply. History owns Git transactions. Processing owns EPUB.
The public functions below are the frozen integration surface for later CLI and MCP wiring.

## Public Surface

`inspect` reads one archive, rejects hostile framing before payload materialization, and hashes every inventoried payload.
`capture` streams caller-supplied bytes into a private archive, validates that archive, and hard-links it only when the destination name is absent.
`stage` copies verified payloads into a new private generation. An existing `INCOMPLETE` generation is returned as interrupted and is not rewritten.
`apply` calls `SafeApply::apply_text_change` or `install_new` and `GitTransactions::admit_inert_bundle`, passing a mutable `VerifiedSource` and the same `Work` to each owner.
`apply` itself does not write the destination or run Git.
This is a breaking Rust interface correction: `apply(staged, storage, history, work)` requires admission for recovery and owner copies; the owner traits accept a source capability, not a path.
`V1Directory::open` verifies genuine retained-v1 directory metadata, mount bindings, complete inventory, native regular files, lengths, and SHA-256 before exposing an inert identity-path core mapping.
`V1Directory::with_file` supplies the same held `VerifiedSource` capability to an owner; `reexport` preserves genuinely supplied context only with unchanged bytes and installs the verified directory atomically without replacement.
The older `v1_import_plan` remains a refusal-only diagnostic for incomplete foreign input; its owner/revision/profile strings cannot finalize a new v1 publication.
`Work` borrows one `knowledge_core::admission::Holding` from `Account::current`. `Holding::grow` runs before each retained buffer or generated write. Scratch that dies inside the call uses a second holding on that same account and `finish`es when the buffer drops. The caller's holding is not finished here. See [OWNER.md](OWNER.md).

Indexes, history bundles, and journals stay inside the private generation. Core and original paths are the only home candidates. Conflicts are not sent to storage.
Private and destination parent directories must be absolute native Unix paths; every ancestor is opened with `NOFOLLOW` from a held directory descriptor.
Returned `Staged` paths are display information, not source authority.
A private authority retains the generation descriptor, operation, snapshot, and conflict facts.
Recovery hashes fresh `MANIFEST` bytes and compares `PLAN` incrementally against that manifest and operation, with an exact EOF check.
An old generation without this binding is refused and preserved, not migrated or deleted.

## Held STORED Source API

`FrozenArchive::open(file, work)` consumes the actual native archive descriptor and verifies all entry CRCs, SHA-256 inventory claims, framing, and the container version.
`with_stored(path, callback)` lends a `StoredRange` with entry-relative `read_at`, `Read`, and bounded `Seek`; no extraction, raw descriptor, or writable handle is exposed.
`EntryRange` reports the checked physical `(offset, length)` for ZIP64 diagnostics.
The reader borrows `Work` exclusively through all callbacks, so its metadata and reader pin cannot outlive the caller's admission holding.
A changed held-container version refuses both access and callback success; replacing a pathname never redirects an existing reader.
`ArchiveVersion` is an opaque equality token for device/inode/length/mtime/ctime, separate from the layout-independent snapshot digest.
`with_indexes(native_requirements, callback)` supplies an actual `IndexDirectory` with inventoried component names, snapshot/version tokens, bounded positional reads, and per-component `Read + Seek` callbacks.
`IndexRequirements` comes from the actual native consumer, never the manifest's `compatible` boolean.
Its checks require the complete STORED index inventory and exact kernel build, lexical/segment format, processor, dictionary, generation, and complete model-space identities.
Optional missing/incompatible indexes return `IndexOmission` without blocking core reads; unknown optional index metadata survives parse/re-export.
These concrete compiled sources are ready for C5's generic byte/generation adapter; they do not themselves claim native kernel search.

```no_run
use knowledge_package::{Error, FrozenArchive, Work};
use knowledge_core::data::RelativePath;
use std::fs::File;

fn read_packaged_component(file: File, path: &RelativePath, work: &mut Work<'_>) -> Result<(), Error> {
    let mut archive = FrozenArchive::open(file, work)?;
    archive.with_stored(path, |source| {
        let mut header = [0u8; 32]; // Caller-owned bounded workspace.
        let _read = source.read_at(0, &mut header)?;
        Ok(())
    })
}
```

Index identity metadata is retained under `indexes.identity` with integer `version: 1`, string `kernel_build`, `lexical_format`, `segment_format`, `processor`, `dictionary`, `generation`, and `model_space`.
A lexical-only `model_space` is null; an exact-vector space supplies `provider`, `model`, `revision`, integer `dimension`, `metric`, `normalization`, `preprocessing`, and `representation: "exact-f32"`.
A matching identity admits component IO, not a claim that the native kernel has opened or searched the component format.
The kernel still owns its real generation/segment validation and query results.

```no_run
use knowledge_package::{Error, FrozenArchive, IndexRequirements, Work};
use knowledge_core::data::RelativePath;
use std::fs::File;

fn native_directory_handoff(file: File, native: &IndexRequirements<'_>,
    component: &RelativePath, work: &mut Work<'_>) -> Result<(), Error> {
    let mut frozen = FrozenArchive::open(file, work)?;
    frozen.with_indexes(native, |directory| {
        let _pin = (directory.snapshot_id(), directory.version());
        directory.with_component(component, |source| {
            let mut header = [0u8; 32];
            source.read_at(0, &mut header)?;
            Ok(())
        })
    })
}
```

## Budgeted Compatibility API Migration

The partial review of `6deb711` identified an unconditional quadratic inventory scan without live control checks.
`index_decision(&self, native: &IndexRequirements<'_>) -> Result<IndexDecision, Error>` and `index_paths(&self, native: &IndexRequirements<'_>) -> Result<&[RelativePath], Error>` replace the old unbudgeted forms.
Use `?` for cancellation, deadline, and admission failures; `Error::IndexOmitted(reason)` remains distinct from those failures.
The implementation inserts/removes each component once in a pre-admitted B-tree with cooperative checks, independent of input order and without a new component or corpus ceiling.
Its scratch holding survives all tree nodes; `with_indexes` uses the same budgeted route before its native callback.
The prior tests changed only the compiled Result/error wrapping; their byte, omission, and pin assertions remain.

```no_run
use knowledge_package::{Error, FrozenArchive, IndexDecision, IndexRequirements};

fn compatibility_names(archive: &FrozenArchive<'_, '_>, native: &IndexRequirements<'_>)
    -> Result<usize, Error> {
    match archive.index_decision(native)? {
        IndexDecision::Compatible => Ok(archive.index_paths(native)?.len()),
        IndexDecision::Omitted(_) => Ok(0),
    }
}
```

## Genuine v1 Directory And TAR Context

The format mapping follows OpenKit's committed retained-v1 directory schema at `30999e395`: `schemaVersion`, `ownerScope`, `knowledgeBaseId`, `notebookRevision`, `requiredFeatures`, `contentInventory`, and single-default-mount `mounts`.
Unknown required semantics, incorrect default/raw bindings, reserved external-mount folders, undeclared files, hardlinks, symlinks, special files, and incorrect digest/length claims refuse.
The native mapping keeps every relative path and authored byte unchanged, including local links, annotations, code examples, schema, and raw digest pins.
Foreign owner/Review labels remain inert data and cannot authorize native roots, publication, or source access.
Unchanged re-export retains the original manifest bytes and verifies private output before atomic no-clobber directory publication.
Changed input refuses and retains private evidence; it cannot carry a false Notebook revision through this operation.
`V1Package` is the general public container; `V1Directory` remains its source-compatible directory-entry alias.
`V1Package::open_tar(File, work)` verifies the original held uncompressed POSIX ustar/PAX file, complete inventory, UTF-8/NFC path closure, case collisions, checked extents, checksums, and complete payload SHA-256 before returning.
TAR members become bounded `VerifiedSource` extents over that descriptor, not an extracted filesystem or a second index.
Link/special/sparse entries, ambiguous PAX controls, missing/duplicate inventory, nonzero trailing carriers, unsupported semantics, and native container drift refuse.
The foreign carrier's 8 GiB selected bytes, 10,000 entries, 1,000,000,000-byte artifact, 16 MiB framing allowance, and 1 MiB manifest bounds only narrow caller quotas; no existing quota is increased.
`reexport_tar` preserves unchanged supplied provenance through an actual verified no-clobber TAR publication.

### Explicit Core And Asset Projection

`V1Package::project(paths, references)` consumes genuine input; `V1Projection::from_native(root, files, paths, references, managed_mount, work)` selects actual native files without fabricating foreign context.
Both return a concrete projection with original/projected byte identities and explicit path/reference tables.
Each selected source retains its actual descriptor and version through preview and copies; changing an outside pathname cannot redirect it.
`copy_file(target, private_writer)` produces the exact planned bytes into an owner's real private writer and checks drift before returning success.
This is the native storage adapter need, not an implementation of M1 transaction authority or an alternate home writer.
Raw originals, schema, code, prose, labels, HTML, comments, line endings, unrelated metadata, and unchanged identity exports remain byte-preserved.
Parsed inline/image/reference-definition destinations and explicit pinned `source:`/`openkit:page/raw` references can be reminted; digest pins cannot be changed by a reference map.
Parsed block-YAML `refs`/`references` and Source `contentRef.locator.path` scalars can change only at confirmed original spans, with current managed-mount selection and comparable raw pins.
A remapped scalar is emitted so a fresh YAML mapping parse of that scalar is the same string.
A plain token is kept only when that parse already yields the string. `null`, booleans, numbers, and other non-string plain forms use `serde_json` double quotes, which YAML reads as strings.
The parser makes that choice. There is no reserved-word list, path-length cap, or input-order rule for it.
Foreign scopes/Review remain inert and are never converted into grants.
Mapped YAML aliases/alias-like tokens, ambiguous scalar spans, and unsupported flow-style remapping refuse before output; unchanged foreign bytes need no YAML interpretation.
A future complete span-aware Source-owner adapter can replace those precise refusals; it must preserve the same identity, byte, and no-grant controls.

### Host-Supplied Finalization And Real Publication

`projection.export(host_manifest: Option<File>, carrier, private, operation, destination)` never fabricates owner/revision/profile strings or invokes a foreign publisher.
`None` returns `V1ExportOutcome::CompatibilityRefused` without opening output roots; `files()` still supplies the actual core mapping preview.
A supplied file is a separate trusted native-host input, not context automatically accepted from imported metadata.
The caller must obtain it from its genuine host publication/finalization and supply its actual schema/profile bytes in the selected inventory; schema-shaped bytes alone do not prove an external Notebook publication.
The package validates that actual descriptor, version, retained-v1 manifest schema, default-mount selection, and exact projected inventory before private effects.
Changed genuine input cannot retain its old Notebook revision; a native Git commit is not substituted for a Notebook publication.
Directory and TAR outputs are reopened and verified before exclusive publication; partial/interrupted output and competing destinations are retained, never replayed or overwritten.
`Published(V1Artifact)` proves only the actual validated filesystem artifact, not owner approval, native M1 saved state, or a new external Notebook publication.
Owned synthetic supplied-context fixtures prove these native byte/version/effect bindings, not an external host service operation.

```no_run
use knowledge_core::data::RelativePath;
use knowledge_package::{Error, V1Carrier, V1ExportOutcome, V1File, V1PathMap,
    V1Projection, V1ReferenceMap, Work};
use std::{fs::File, path::Path};

fn native_host_export(root: &Path, selected: &[V1File], paths: &[V1PathMap],
    refs: &[V1ReferenceMap], current_managed_mount: Option<&str>,
    finalized_host_manifest: Option<File>, private: &Path, output: &Path,
    work: &mut Work<'_>) -> Result<V1ExportOutcome, Error> {
    let mut projection = V1Projection::from_native(root, selected, paths, refs,
        current_managed_mount, work)?;
    projection.export(finalized_host_manifest, V1Carrier::Tar, private, "host-export", output)
}
```

```no_run
use knowledge_package::{Error, V1Directory, Work};
use std::path::Path;

fn unchanged_foreign_round_trip(input: &Path, private: &Path, output: &Path,
    work: &mut Work<'_>) -> Result<(), Error> {
    let mut package = V1Directory::open(input, work)?;
    package.reexport(private, "unchanged-export", output)
}
```

## Bounds

Callers pass Core `Budget` and explicit format quotas. Zero closes the call.
`Work.files` enforces Core's examined-object ceiling before native access, including unavailable objects.
Each native root/component or file/marker observation consumes one unit; reusing a descriptor for a fresh verification or exact-copy pass consumes another unit, not one per read chunk or syscall.
Recovery therefore counts markers, `MANIFEST`, and `PLAN`, not only restored payloads.
Apply also checks that a copy unit remains before delegating to storage or history; `VerifiedSource::copy_to` admits its own pass before examining the descriptor or writing output.
Exhaustion returns typed `Error::Quota` without increasing the counter beyond the supplied ceiling.
The caller budget is not raised, reset, or replaced during delegation; earlier completed owner writes remain preserved.
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

### Native Held-File Owner Input

`VerifiedSource::with_private_file` copies the selected stream, then opens that copied name `O_RDONLY` and `O_NOFOLLOW` while the writing descriptor stays open.
The opened descriptor must match the copied device, inode, length, and timestamps, and its file-status access mode must be read-only.
That same descriptor is rehashed before the owner callback. The writing descriptor stays open until the callback returns.
On Linux, `/proc/self/fd` of the writer is an additional identity check. It is not the handed capability, and a missing `/proc` entry is not `Unsupported`.
The same open is compiled for macOS. This crate does not execute macOS and does not claim that observation.
The handoff does not expose the original archive descriptor and does not finish the caller's holding.
`import_verified_source` calls `Store::import_copy` with the read-only copied descriptor, the caller's `Preflight`, and the current `WritePolicy`.
`install_verified_text` admits the cached source length, text, digest encoding, and request copies before any path copy or private handoff, then calls `Store::prepare` and, only when preparation is new, `Store::install`.
It does not call `publish`.
`verify_inert_bundle` and `admit_inert_objects` pass that descriptor and its recomputed digest to the caller's `History` and the same `Claim`.
Object admission does not activate refs, checkout, config, hooks, remotes, or trust.
`knowledge_history::Error::Unsettled` is returned as that value, or parked on `StorageOwner` or `HistoryOwner` until `take_unsettled`.
Display text is not collection and is not claim settlement.
`export_ordered_epub` passes `HeldMember` values, in slice order, to `export_epub_held_funded`.
Logical names do not select files, and this crate does not reopen a source path.
`EpubOwnerError` keeps partial descriptors, live holdings, an unsettled parser child, or the original admission error.
C5 `NativeSource` and `Delivery` stay on the root service at `71290207`. This package does not depend on that root crate.
The caller supplies the current store, history, policy, and claim. This package does not invent an approval record.
Active M1 tracking recovery `0fc` is not edited here and is not qualified. No Apple SDK is installed.
`History::verify_signature` reports Git object cryptography. It is not an SSHSIG check of the decoded snapshot digest, and it is not a revocation policy.
`OwnerHandoff<T>` always preserves the real native owner result and reports post-callback cancellation or drift separately.
The initial source milestone is `833fb93`. This guide does not waive any requirement in the canonical [package owner](../../docs/specs/package.md).

```no_run
use knowledge_core::data::ContentDigest;
use knowledge_package::{Error, OwnerHandoff, VerifiedSource, Work};
use std::{fs::File, path::Path};

fn held_native_input<T>(source: &mut VerifiedSource, private: &Path,
    operation: &str, work: &mut Work<'_>,
    native_owner: impl FnOnce(&File, &ContentDigest, &mut Work<'_>) -> T)
    -> Result<OwnerHandoff<T>, Error> {
    source.with_private_file(private, operation, work, native_owner)
    // Native result and post-callback source_revalidation must both be inspected.
}
```

## Necessity And Ablation

| Mechanism | Present need | What removal loses |
| --- | --- | --- |
| Owned framing reader | Reject traversal, links, encryption, overlap, and ZIP64 disagreement before copy. | The mature ZIP writer would still accept hostile input through a permissive extract path. |
| Streaming native ZIP and extent gate | Avoid retaining payloads and a rendered archive while admitting every new native extent. | A payload-only estimate misses framing and compression growth. |
| Node/string preflight and live metadata guards | Fund real serde/JCS/semantic representations before allocation. | Many small optional elements can cost much more than their encoded bytes. |
| Held directories and `VerifiedSource` | Keep confined native source identity through owner delegation and exact-stream verification. | Hashing then passing a mutable path proves a different read. |
| Held TAR member table and spans | Read genuine POSIX/PAX payloads directly with format/caller bounds. | Extracting first loses container identity and creates unnecessary filesystem authority. |
| Explicit parsed core/asset projection | Remap verified identities and destinations without changing prose/raw bytes or losing pins. | Text replacement changes code/unrelated metadata and can forge digest relationships. |
| Host-file finalization and reopened output | Bind actual selected bytes/profile to separately supplied context before publication. | DTO strings fabricate a Notebook publication or stale foreign revision. |
| Budgeted component membership | Check uniqueness/coverage in cooperative linearithmic work with live scratch. | A cancelled valid large inventory can continue quadratic CPU work. |
| Native read-only same-inode handoff | Supply the actual native owner File API from the descriptor that received the copy, and preserve its independent result. | Reopening a directory name, or hashing a descriptor and then using that path, proves a different file. |
| Actual expand counter | Stop bombs when output exceeds the caller quota or ratio. | Declared sizes would choose the buffer. |
| `Holding::grow` / `written` | Charge the one process account before retained buffers and generated writes. | A private counter could admit work the account refused. |
| Scratch `finish` | Release a dead inflate window without releasing retained names or output. | Every header would stay charged until the host finished the whole call. |
| Private generation plus owner traits | Keep a previous complete generation and let storage and history perform the final write. | This crate would grow a second rollback and Git engine. |

No registry or workflow engine is added in this slice.
Same-core import, text install, inert object admission, and ordered held EPUB encoding are linked on Linux.
Root now composes actual native detached-signature trust and STORED kernel activation through `NativeTrustedArchive`; installed product routing, macOS execution and an external host publication receipt remain separate acceptance obligations.
`Signature::Unverified` is not trust. A Git `ValidKnown` result is not detached snapshot trust for namespace `omni-knowledge.snapshot.v1`.
`required_features` still refuse installation. Supplied context fixtures are not external Notebook acceptance.

## Checks

```sh
cargo check -p knowledge-package --locked --offline
cargo clippy -p knowledge-package --all-targets --locked --offline -- -D warnings
cargo test -p knowledge-package --locked --offline
cargo doc -p knowledge-package --no-deps --locked --offline
```

## Native indexed capture and detached snapshot verification

`capture_with_indexes` uses the same streaming capture encoder with explicit producer `IndexRequirements` and the actual complete selected Role::Index inputs.
It requires a nonempty index set, preserves legitimate zero-length STORED components and emits digested component inventory plus complete native identity data.
It does not validate the index engine's binary format or grant trust; the consuming kernel independently validates that format and all required identities.
Ordinary `capture` retains the explicit unqualified-index omission behavior.

`FrozenArchive::verify_manifest_signature` reads bounded detached signature bytes from the same pinned verified archive and calls `History::verify_snapshot_signature` with the actual native owner and live HistoryWork.
The signature covers the decoded canonical snapshot digest in namespace `omni-knowledge.snapshot.v1`.
`ManifestVerificationError` preserves the actual typed native unsettled child owner for collection.
The framing `Signature::Unverified` value alone still grants nothing; native Root selects known-key or explicit-unsigned policy and retains its current selection and Claim through later reads.
`guarded_reader_metered_capped` borrows only its validated finite verifier cap from the caller's real Holding and charges aggregate read admission before archive I/O.
It neither raises package bounds nor creates another account.
