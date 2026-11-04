# Preparatory Private Cache Mechanism

Status: builder implementation; independent cache tests and consequential key review pending.
Owner: [storage identity, cache, and recovery](../specs/storage.md).
The engineer assigned a new branch from local core `28b29c659d328f02d481830043bf6ee2c8657473`, preserving that framework, its independently used Rustdoc, and wire `921ab9c9a80438d7a9de61dd600e9e1383389b2b`.
This is not M0 acceptance, complete M1, usable application integration, or a change to the accepted owner.

## Scope And Public Effects

The new Linux/macOS `knowledge_core::cache` module implements actual private local derived-object installation and current-source-mapped pinned reads.
The source lease includes only cache source, its minimal module export, necessary already-locked crate dependency/lock declarations, a cache-only crate guide section, and this record.
No existing data/manifest/backend API, manifest example, lasting tests, root workspace manifest, vendor, workflow, or owner changes.
There are no application callers, file adapters, service/authorization cutover, provider integration, agents, messages, or push.
The [crate guide](../../crates/knowledge-core/README.md#preparatory-private-cache) documents API usage, ownership boundaries, and limitations.

## Mechanism And Responsibility

The trusted native owner provides a selected existing app-owned cache root, complete current prohibited-root facts, locality/ACL assessment, and current admitted library/source/processor mapping.
The module checks native ownership/mode, no-symlink path components, captured root identity, lexical and physical-ancestor nonoverlap, and source-home binding before writes.
Each operation revalidates these facts; opened root/namespace/fanout/file descriptors attach effects to captured identities with no ancestor fallback.
The trusted mapping boundary is not a portable permission token and does not implement M6 authentication or local exposure approval.
Current source digest and effective input-dependency verification remain with that admitted source/processor owner.
No method opens an original or invents an asset identity from equal digests.

Processor identity uses deterministic length-framed hashing, explicit optional presence, sorted maps, immutable code/model/package digests, and effective version/options/space/dimensions/preprocessing/dependency data.
Each identity has a version/domain salt; the object key binds two fixed-width SHA-256 identities without ambiguous boundaries.
Numeric option spellings stay exact strings, never rounded floating-point JSON values.
Options include effective defaults and overrides; corrected transcript and embedding-relevant annotation digests must be explicit used dependencies.
Namespace hashing separately binds selected native library-home identity, library ID, and sensitive/unexposed/exposed domain.
Neither those data identities nor constructor success grant service access.

Actual derived bytes stream through a finite caller-selected buffer and disk/object limits into an exclusive private candidate.
After complete header/payload SHA verification and read-only permissions, an atomic same-directory hard link creates the complete name without clobbering a competitor.
Existing complete names are independently verified, not overwritten; conflicting payloads and corrupt objects remain visible failures.
Post-install binding/sync uncertainty is Indeterminate; no rollback, unlink, silent overwrite, or automatic replay occurs.
Pinned reads verify the entire bounded payload while retaining only the requested bounded range, and compare before/after file/root/current-mapping facts before delivering bytes.

One native owner lock excludes other cooperative services; reader pins retain its semantic lifetime.
Explicit guard unlock precedes descriptor close, addressing the inherited-description lifetime concern without a process manager.
Post-fork use refuses before acquiring an inherited Rust mutex; forked destructors do not unlock the parent's description.
One nonblocking cache I/O/accounting lane bounds active module-owned buffers; an explicit pin cap bounds retained handles/recipe copies.
Contention or reentrant producer calls return Busy, rather than blocking behind themselves.
The caller still owns producer wait bounds, context allocation, returned buffers, and global process/job resource policy.

## Deferred Lifecycle And Qualification Limits

All candidates, including failed and successful ones, remain private and charged; successful canonical/candidate hard links are conservatively charged twice by logical name size.
No file is ever deleted here, so complete objects and all intended reader pins are preserved without inventing incomplete cross-process eviction accounting.
Unknown files and symlinks remain with warning counts; an unknown directory requires inspection rather than concealing uncharged descendants.
Reopen reaccounts complete/partial data but does not resume jobs, adopt ambiguous reservations, reclaim candidates, retire generations, or update coverage after eviction.
Disk accounting includes logical file/link sizes and entry bounds, not physical allocation overhead or reservation against external programs.
A fresh owned GC/reservation/restart slice must supply those missing mechanisms before integration requires reclamation; this record tracks the scope, retained-space risk, receiver, and removal condition.
No complete owner-lifecycle claim is made.

Unresolved/offline or symlink-spelled prohibited roots currently prevent admission because this implementation cannot establish physical nonoverlap for them.
That limitation must be reviewed against the complete accepted offline behavior before integration; it is not an accepted narrowing or a guessed safer root policy.
The module verifies Unix UID/mode bits; trusted native locality, extended ACL/inheritance, and complete root discovery remain caller obligations.
Native macOS ACL/filesystem/lock/directory-sync behavior, inherited-descriptor lifetime, syscall interruption, storage faults, and adversarial substitutions require independent real-file/native evidence.
Descriptor confinement and before/after checks do not establish universal atomicity against arbitrary malicious same-UID namespace movers or privileged remounts.
No unsafe code, cache detector/plugin ecosystem, source-body journal, or generic scheduler is introduced.

## Necessity And Reasoned Ablation

| Mechanism | Required responsibility | Consequence of removal |
| --- | --- | --- |
| Structural salted processor/key hashing | Bind all effective processing inputs without numeric loss or ambiguous concatenation. | Stale or unrelated derived outputs can collide or be reused. |
| Trusted current roots/source boundary | Separate data identities from current admission and known locations. | Naked hashes or portable manifest claims become access authority. |
| Native descriptor walk/binding checks | Confine actual effects and detect observed root/node substitution. | Pathname fallback can redirect writes/reads or follow escaping symlinks. |
| One native owner guard with explicit unlock | Bound supported reader/writer ownership across processes and actual intended lifetimes. | Another service can violate accounting/pins, or unrelated inherited descriptors prolong ownership. |
| Nonblocking I/O lane and bounded pin count | Bound module-owned active buffers, retained descriptors, and recipe copies. | Per-call bounds permit unbounded simultaneous cache-owned resources or self-deadlock. |
| Exclusive candidate and no-clobber hard-link install | Publish only complete verified bytes without replacing a prior complete object. | Partial visibility or unconditional replacement violates preservation. |
| Retained candidates plus bounded full accounting | Avoid deleting unknown/recovery material before owned reclamation exists. | Space is hidden or cleanup guesses ownership and can delete unrelated material. |
| Fully verified mapped range pins | Return bounded bytes only from current complete objects. | Changed mapping/file content can escape through stale cached handles. |

The only added dependencies are exact already-locked rustix 1.1.5 (native fs/process helpers, Apache-2.0/MIT) and sha2 0.10.9 (SHA-256, Apache-2.0/MIT).
No new external package/version upgrade or handwritten unsafe syscall layer is needed.
No uncertain removal/performance claim is made; no measured experimental ablation is claimed.

## Evidence And Next Owners

Builder evidence uses separate owned target/TMPDIR/dictionary state and exact final committed-head focused format/check/strict all-target Clippy/documentation and public-cache disposable-file smoke.
The external probe links the actual public crate and uses real cache files/locks/processes, not a fake filesystem or cache backend.
Its synthetic trusted mapping/locality premise is not independent native source verification, cloud/NAS detection, or service authorization evidence.
The assigned cache log records exact commands, identities, failed/partial observations, dependency deltas, residual fixtures, and final-head results.
No lasting tests are written by this builder; the independent core test author retains ownership of those paths and the old public artifacts.
Independent cache cases and consequential key review must follow before integration; compile/example/builder smoke results cannot self-accept this mechanism.
