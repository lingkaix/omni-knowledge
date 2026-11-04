# Knowledge Core

This crate owns pure mutable-manifest inspection/name editing and the neutral ordinary mount-backend seam.
It does not initialize, migrate, discover, activate, authorize, or persist anything.
The [storage owner](../../docs/specs/storage.md) governs behavior; [product](../../docs/specs/product.md) assigns module responsibility, and [service](../../docs/specs/service.md) owns the separate wire boundary.
This preparatory slice does not complete M1 or establish M0 acceptance.

## Public Surface

- `manifest::ManifestDocument::decode(bytes, ManifestLimits)` admits one bounded UTF-8 JSON object.
- `original_bytes()` returns the exact admitted bytes; `field_json(name)` returns an uninterpreted raw field value.
- `state()`, `version()`, and `metadata()` distinguish supported version 2, legacy version 1, unsupported versions/semantics, and malformed recognized fields.
- `with_name(name)` makes only a pure display-name candidate and refuses every read-only state.
- `backend::MountBackend` exposes capabilities, bounded list, non-hashing stat, and expected-version range reads.
- `data` contains the shared ordinary backend kind, checked relative paths, and checked claimed SHA-256 values.

There is no dependency on the unaccepted `omni-service` slice, no command union, and no application caller or authority cutover.
Backend facts use native Rust counts and signed nanosecond observations rather than service U64 strings.
Availability is `Available` or a typed `Unavailable` reason; the service must explicitly map domain facts to its flat wire tags.
Public data structs and constructors assert facts only; they issue no verification, residency, disclosure, root, transport, write, or hydration grant.
A supplied digest is not proof that an admitted source was actually hashed.

## Manifest Preservation And Mutation

The required top-level fields are version, library_id, name, text_root, mounts, exposure, and convention_version.
Version 2 requires one pages/wiki text root, nonempty opaque library/mount IDs, and unique single-component mount names.
IDs have no UUID restriction.
The mount projection reads mount_id, name, backend, location, writable, and reconciliation.
Backend kinds and reconciliation tags follow the service data owner; location remains inert opaque text.
The exposure projection reads mode, sensitive, excluded_paths, and transports, without service revision/pending-approval state.

The codec preserves the original byte stream, including whitespace, order, escaped keys, raw numeric spellings, and opaque extensions.
Legacy version 1 and unknown required versions/known semantic tags remain inspectable without migration, defaults, mount contact, or provider interpretation.
Malformed recognized fields have an explicit read-only state; syntax/UTF-8/outer duplicate-key/budget failures return fixed errors.
Duplicate decoded keys are rejected in the outer object and in interpreted mount/exposure objects.
Unknown optional nested values remain opaque raw JSON, including their internal keys; this codec does not choose their meaning.

An unchanged supported name returns a byte-exact copy.
A changed name normalizes only the outer object's key encoding/order/spacing and the changed name encoding.
Every other raw field value remains verbatim, including nested unknown fields and large numbers.
It does not round-trip through a floating-point JSON AST.
Known metadata snapshots omit extensions and must never be serialized as replacement manifests.
Future filesystem owners must compare current expected bytes, preserve competing versions, and install candidates safely; this crate performs none of those steps.
It supplies no mount/exposure editor, mount rename/alias map, path migration, initialization, or template writer.

### Interpretation Requiring Independent Review

Storage specifies OKF 0.2 and the convention_version field, but not its exact JSON encoding.
This projection recognizes the string `"0.2"`; different encodings stay verbatim read-only rather than being converted or rejected as malformed solely for their version.
The writable/reconciliation manifest field projection also uses the service owner's current names because storage does not enumerate a full JSON manifest example.
These are explicit implementation projections, not amendments to the owner or proof that every valid version-2 convention is editable.
Independent owner/public-API scrutiny must resolve these details before persistent integration.
No unknown-field extension registry, required-feature list, critical-field marker, or imported executable instruction protocol is invented.
An unknown optional field is preserved, not executed or treated as a grant.

## Bounds And Errors

Callers select an explicit finite input/output byte ceiling; zero admits nothing.
There is no new fixed on-disk manifest size limit.
A 128-level nesting ceiling bounds the raw-value parser and remains an implementation safety limit, not a claim that deeper files are corrupt.
Candidate encoding checks its ceiling before appending a chunk, including JSON escaping expansion, and returns no partial candidate.
Memory is bounded by a constant-factor number of input/candidate copies plus parsed recognized metadata; this is not measured RSS or a global process-memory guarantee.
Errors and their source chains contain no rejected input, body, path, Serde payload, or credentials.
Neither the codec nor preservation is a secret scanner: callers must supply only authorized bytes, and credentials/settings do not belong in a portable manifest.

## Backend Contract And Evidence Limits

Instances must be bound by a real adapter to an explicitly selected root and its current identity.
Capabilities are runtime observations, not portable declarations or service permissions.
The five flags describe range reads, qualified stable IDs, reliable watch, conditional writes, and tested residency.
Watch/write signatures, tracking, and write engines are deliberately not frozen speculatively.

Listing has distinct complete and partial variants, explicit stop reasons, bounded progress, and optional backend-qualified cursors.
Offline/denied/unknown roots return classified unavailability, never a successful empty complete listing.
Partial or cancelled enumeration cannot establish absence or deletion.
A cursor is observation state, never asset identity, a new root, or permission.
File-ID evidence retains backend/library/mount plus available volume, generation, and birth facts; equal digests never merge asset identity.
Stat carries size, high-resolution modification/change times, provider version, residency, and freshness without hashing.
ExpectedVersion binds a claimed verified digest to those backend/path facts; range reads cannot resolve a naked hash.
Before/after version checks must refuse detected drift rather than return mixed bytes.
Unknown cloud residency and placeholders remain metadata-only without separately established selected-file read/hydration admission.
No public data value supplies that admission.

ByteRange checks nonzero length and end overflow, not access permission or allocation size.
The actual adapter must enforce each supplied file/byte/time/memory budget, metadata and cursor bounds, confinement, child ownership, residency, current exposure, and cleanup.
There is no adapter/default method/fake backend here, so no actual I/O enforcement, native residency, NAS continuity, or conditional-write proof is claimed.

## Necessity And Reasoned Ablation

| Mechanism | Present need | What removal loses |
| --- | --- | --- |
| One real pure codec/editor | First owned version-aware behavior without authority cutover. | No actual reusable manifest operation; an empty crate or DTO-only shell. |
| Raw field values plus original bytes | Preserve authored extensions and exact read-only input without numeric loss. | Typed reserialization loses extensions/precision; ordinary JSON ASTs can rewrite raw values. |
| Required-field interpretation and read-only states | Refuse unknown required versions/semantics without discarding readable data. | Future/legacy data can be silently mutated under guessed rules. |
| Caller byte ceiling, depth ceiling, capped writer | Bound parse/encode work and escaped output without a new storage-format quota. | Oversized input/nesting/output can consume unbounded resources or leak partial candidates. |
| Checked path/digest/range data | Share only actual lexical invariants at current domain boundaries. | Unsafe path spellings, malformed claimed hashes, or overflowing ranges reach adapter callers unnoticed. |
| One backend trait and qualified observations | Enable ordinary adapters while keeping identity, unavailability, and completion explicit. | Adapter-specific parallel seams or offline-as-empty/unknown-as-resident ambiguity. |
| Locked Serde/serde_json with raw_value | Mature JSON grammar/encoding and raw-value retention. | A new handwritten JSON parser or lossy extension handling. |

No registry, callback system, generic workflow, second service catalog, hidden database, or speculative adapter is necessary.
No uncertain removal claim requires an experimental ablation in this additive slice.
The interpretation questions above remain review limits, not experimental performance claims.

## Builder Commands And Next Owners

Use a separate target, TMPDIR, two Cargo jobs, and the repository's verified dictionary-supply procedure before root Cargo commands.
Run the focused commands from the repository root:

```sh
cargo check -p knowledge-core --locked --offline
cargo clippy -p knowledge-core --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo test -p knowledge-core --all-targets --locked --offline
cargo build -p knowledge-core --example manifest --locked --offline
cargo doc -p knowledge-core --no-deps --locked --offline
```

The manifest example accepts bounded synthetic stdin, optionally `--rename NAME`, and emits the real public codec's preserved bytes/candidate without a trailing delimiter.
Its 1 MiB ceiling is an example choice, not the file format.
Examples, compiler checks, and builder smoke are not independent acceptance cases.
A separate test author must derive lasting tests from accepted owners and committed public API only, before independent key review.
No existing tests were rewritten or retired.

Suggested separate follow-on leases are a confined local backend, an existing-OS-mount NAS adapter, and a tested cloud-folder residency adapter.
Each needs its own independent public-seam tests; cloud/NAS/native effects require actual authorized environments.
Initialization/migration, journal/tracking, safe writes, cache, history, packages, processing, service integration, settings/approval, and application cutover remain unimplemented here.
See the [change record](../../docs/changes/2026-11-04-core-framework.md) for assignment and evidence scope.

## Preparatory Private Cache

The earlier scope statements describe the preserved manifest/backend slice.
The new Linux/macOS `cache` module adds derived-object I/O only; it does not activate that slice or change its APIs.
The [cache record](../../docs/changes/2026-11-04-cache-framework.md) records necessity, limitations, and required independent follow-up.

### Trusted Inputs And Placement

`Cache::open(existing_root, context, limits)` requires an already provisioned app-owned 0700 directory.
It does not guess an OS cache location, create a parent, follow symlinks, or repair unknown permissions.
Admission checks current-user ownership, exact mode bits, captured native identity, lexical overlap, and physical ancestor identities before creating even its owner file.
Every effect/read rechecks current roots and source mapping, and held descriptors keep effects attached to captured roots and namespace directories.
Changes refuse access; they never redirect an operation to a parent.

`CacheContext` is the one trusted in-process mapping boundary, not a serializable approval token or provider/processor registry.
Its native owner must supply complete current home/mount/cloud/history roots, independently observed locality, and private ACL/inheritance assessment.
Mode checks establish Unix ownership/mode facts, not an independent extended-ACL or cloud-provider assessment.
`Untrusted` locality always refuses; `ExplicitLocalAlternative` still requires native observation for that actual selected root.
This module does not infer locality or permission from a manifest, filesystem name, or ID.
Prohibited paths must currently be accessible absolute no-symlink directories; unresolved/offline roots fail closed rather than being assumed outside the cache.
That current limitation is not approval to narrow accepted offline behavior; owner scrutiny is required before integration.

`resolve(library, source, effective_processor)` must recheck current selected root, verified original digest, exclusions, local policy, and every effective dependency.
A `SourceAdmission` returned by that trusted owner binds the native selected-home identity and local admission revision, not only a portable library ID.
A copied same-ID library has another namespace; a moved same-inode library can reuse compatible facts only after current validation.
Sensitive, unexposed, and exposed domains have distinct per-library namespaces.
The cache exposes no naked-hash lookup, original-copy operation, filesystem adapter, service authentication, or exposure approval.

### Keys And Real I/O

`ProcessorIdentity::fingerprint(max_bytes)` structurally hashes code digest, effective version/options/model/package/space/dimensions/preprocessing, and dependency-role digests.
The processor owner must expand defaults/overrides and include actual immutable artifacts plus every used corrected transcript and embedding-relevant annotation.
Map keys sort deterministically; values are exact UTF-8 strings, not floating-point JSON numbers or ambiguous concatenations.
Meaningful order belongs in the effective value; equivalent numeric spellings may conservatively produce different identities rather than collide through rounding.
Never put credentials or source bodies into identity options; use the appropriate input digest.
Distinct processor/object/namespace version salts prevent cross-domain reinterpretation.
The object key binds the verified original digest and processor fingerprint, without inventing a source ID from equal bytes.

`publish` streams actual derived bytes with finite buffer/object/disk/entry settings, verifies the complete candidate, and atomically hard-links its complete name without replacement.
The old complete object is never overwritten, renamed away, rolled back, or deleted.
Identical existing objects are independently verified; differing payloads at an identical key are conflicts, and corrupt objects are refused.
Directory sync/binding failures after a possible install return `Indeterminate`, not fabricated success or automatic replay.
The producer must provide derived output only and bound its own blocking I/O; EOF must mean validated complete processor success, not merely a failed child's closed pipe.
Cancellation is observed between bounded chunks and before the atomic install; cancellation after that commit point cannot undo a complete object.

`pin` resolves the current admitted source and opens a real complete object; `PinnedObject::read` revalidates that mapping and returns only a bounded range from one fully SHA-verified stream.
Detected root, policy, dependency, source, namespace, or native file-version drift returns no bytes.
One nonblocking I/O lane bounds cache-owned active buffers/accounting; concurrent or reentrant operations return `Busy`, without a hidden queue or deadlock.
`max_pins` bounds retained reader descriptors and recipe copies; returned caller-owned byte buffers, context allocation, global RSS, and producer execution remain outside those per-operation bounds.
Active RAM has a constant-factor buffer/range bound plus bounded retained request data; it is not a measured RSS or physical-memory ceiling.

### Ownership And Deliberately Incomplete Lifecycle

An actual nonblocking native file lock enforces one cooperative owning service for the entire selected root.
Pins retain that semantic owner after the Cache handle drops; cross-process cache readers are not offered.
The guard explicitly unlocks before closing, so unrelated inherited pre-exec descriptors do not intentionally prolong the reader lifetime.
Post-fork cache use refuses before touching an inherited Rust mutex; child destructors do not unlock the parent's shared description.
Independent native inherited-descriptor regression and macOS qualification remain required.

No file is deleted in this slice, including successful/failed candidates, unknown files, symlinks, and complete objects.
Usage reports retained-candidate and unknown-entry counts as inspection warnings; unknown directories stop accounting instead of concealing an unbounded subtree.
Hard-linked complete/candidate names are conservatively charged twice by logical size; filesystem metadata/allocation overhead is not a physical-volume reservation.
Capacity exhaustion stops publication and leaves prior complete objects readable.
Complete objects can reopen under a new owner; partial candidates stay private, inspectable, and charged across restart, with no job replay or automatic reclamation.
This is intentionally not full GC, coverage-after-eviction, reservation/restart recovery, or the complete storage lifecycle.
Those omissions block owner-completeness claims and require a fresh owned key slice before integration that needs reclamation.

Native mode checks and descriptor confinement protect the admitted cooperative private namespace, not arbitrary same-UID malicious code, privileged remounts, forged cache files, or atomicity across namespace movement and policy updates.
Before/after checks detect observed substitutions; they cannot prove no adversary moved the root into a prohibited tree during a syscall.
Never treat that limitation as permission to follow symlinks, delete unknown nodes, or publish through unvalidated roots.
Use only disposable real local files for builder probes; lasting independent cache tests and consequential key review must precede integration.
