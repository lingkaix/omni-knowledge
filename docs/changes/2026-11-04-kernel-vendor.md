# Pinned Kernel Dependency And Vendor Proof

## Intent And Boundary

The accepted [product contract](../specs/product.md#shared-crates-and-platform-boundaries) requires one reused lexical kernel, not a second permanent production implementation.
M0 must prove the public pinned dependency with a real downstream build, then retain its exact source and the evidence needed for a selected path dependency.
The source revision is [OpenKit b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1](https://github.com/lingkaix/openkit/tree/b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1/packages/retrieval-kernel).
No OpenKit change, application workspace change, search cutover, GUI, model runtime, or archive-reader implementation is part of this landing.
The workspace owner remains responsible for application membership and its actual dependency lock.
The retrieval owner remains responsible for the production generation identity and the later single-engine cutover.

## Artifacts And Provenance

- [Vendored crate](../../vendor/retrieval-kernel/VENDOR.md), with every original crate file retained and per-file original/retained SHA-256 in [PROVENANCE.json](../../vendor/retrieval-kernel/PROVENANCE.json).
- Exact upstream Apache-2.0 root license, original supplemental notices and provenance inventories, dictionary archive and COPYING notice.
- Exact [upstream root lock](../../vendor/upstream/openkit-b67cd0f2.Cargo.lock), SHA-256 `88be6b21f6430d1aa5bf3b730822905e37ed7514a2378a49b454265797a0947a`.
- [Isolated downstream workspaces](../../tools/kernel-probe/README.md), each with its actual Cargo-generated lock and the same public-API consumer.
- A standalone vendor lock for the retained Rust test/build resolution and a [replayable two-patch delta](../../tools/kernel-probe/patches/m0-vendoring.patch).

The Git dependency is `retrieval-kernel = { git = "https://github.com/lingkaix/openkit", rev = "b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1" }`.
The selected vendored consumer instead uses `path = "../../../vendor/retrieval-kernel"` from its isolated manifest.
Neither probe adds a dependency to the application production graph.

The compiled resolutions use Tantivy 0.26.2, tantivy-jieba 0.21.0, jieba-rs 0.11.0, Lindera 5.0.1 and lindera-ipadic/lindera-dictionary 5.3.0.
The Git lock has 261 package entries and the path lock has 260; the source identity and consumer package differ.
Newly resolved transitive licenses require their own redistribution audit; retained upstream notices are not that audit.

## Minimum Adaptations And Ablation

| Mechanism | Present necessity | Reasoned ablation |
| --- | --- | --- |
| Vendor copy | The pinned public kernel has filesystem-only private source/generation interfaces, but the accepted build requires mount/archive readers. | Without the copy, those required interfaces cannot be adapted here without changing OpenKit. Keep a single selected production vendor dependency; the Git probe remains historical evidence only. |
| Upstream-lock include relocation | The original relative include resolves to Omni's unrelated root lock after moving the crate. | Removing the relocation silently changes what the upstream stamp means. Retain the original lock bytes and bind the actual production consuming lock separately at integration. |
| Fixed-array digest iterator | Rust 1.99 Clippy rejects the original `chunks_exact(2)` loop under `-D warnings`. | Removing this mechanical adaptation restores that observed strict-lint failure. The prior 64-byte validation preserves the same pair decoding; no warning suppression or test change is needed. |
| Isolated consumers and actual locks | M0 needs executable downstream proof while the lead owns the application workspace conversion. | Source inspection alone cannot establish that the public Git dependency compiles or that its vendored path links. Separate workspaces keep both actual resolutions reproducible without adding two production engines. |
| Verified dictionary cache supplier | Lindera can download dictionary data from its build script even with Cargo offline. | Without verified seeding, the build depends on an implicit external archive fetch and unverified cache bytes. Reuse the pinned upstream supplier with only its default archive path and documentation adapted. |
| Source hash inventory and patch ledger | An upgrade must distinguish exact retained source from local adaptations and preserve license/dictionary evidence. | Without these records, reviewers must reconstruct the delta and cannot cheaply check exact provenance. No new registry, service, format authority, or plugin mechanism is introduced. |

M0 changes no analyzer, ranking, canonical byte, scan-coverage, publication or test-assertion semantics.
The vendor Rust tests pass before the iterator adaptation; the final targeted gate reruns them after it.
No test is deleted, weakened, replaced, skipped, or retired.
The downstream probe is builder-authored feasibility/characterization evidence, not independent M4 parity acceptance.

## Direct Proof

The build host is Linux x86_64, Rust `1.99.0 (b940084d7 2026-09-28)`, Cargo `1.99.0 (5f94df478 2026-08-27)`.
Builds use a dedicated target directory, a dedicated verified dictionary cache, and `CARGO_BUILD_JOBS=2`.
The [probe guide](../../tools/kernel-probe/README.md) gives the exact repeatable Cargo commands and prerequisites.
The executed consumer publishes synthetic files and queries the real linked kernel through public operations.
Its assertions cover English/Chinese/Japanese lexical retrieval, overlapping exact canonical offsets, LF-line regex, reopened generations, no-op publication and stale-source withholding.
The selected Rust suite contains 48 vendor library tests and seven executable-face tests, with no ignored or filtered cases.
Each consumer also has one real downstream contract test.

A deliberate wrong exact offset in an isolated temporary consumer fails at the deciding assertion with exit 101.
The negative copy uses a distinct package identity so Cargo recompiles it instead of reusing a same-name cached artifact.
The initial same-name negative copy reused an artifact and returned an invalid green; that observation is not counted as detector proof.
Damaged dictionary supply fails before creating its cache directory.
An initial 60-second cold-build smoke expires while compiling dependencies; it is a setup limitation, not a failed retrieval assertion.
The subsequent locked Git build and executed consumer pass.
Testing the vendor through the nonmember probe dependency fails Cargo setup; direct vendor-manifest testing initially supplies the corrected route.
The concurrent parent-workspace conversion later makes direct tests require workspace membership or exclusion.
Until that owner resolves membership, the final vendor gate uses an exact final-HEAD source/manifest/lock snapshot without the unrelated parent manifest; path-consumer compilation remains in the actual checkout.
No source or test changes are made in that snapshot, and this does not claim application integration.
The original Rust 1.99 strict Clippy failure and its mechanical correction remain recorded, rather than suppressed.

## Integration Requirements And Remaining Work

[The vendor ledger](../../vendor/retrieval-kernel/VENDOR.md#required-seams-characterized-not-implemented) identifies the exact source obstructions and smallest generic upstream candidates.
Required future seams are an admitted byte reader, immutable read-only generation/Tantivy Directory provider, caller budgets, bounded streaming inventory/shard planning and explicit consuming dependency identity.
A read-only archive provider must replace manifest/sidecar access and filesystem reader pinning as well as Tantivy index opening.
An ordinary file range or inline payload alone does not provide this archive seam.

The upstream machinery stamp still hashes the retained upstream lock, dictionary and settings.
It does not bind the consuming application's actual resolved graph automatically.
Before production reuse, bind the actual application lock digest, resolved feature/target identity, vendor source/patch fingerprint and dictionary identity into publication/open/query compatibility checks.
A mismatch must reject reuse and require a rebuild before kernel access.
Do not overwrite the upstream evidence or infer a lock location from the process working directory.

No frozen-ZIP direct search, arbitrary-library-scale support, Mac/arm64 qualification, full application regression, built-image notice audit, real-model quality or production cutover is claimed.
The copied upstream Python notice/image tests require upstream collectors/image infrastructure and are not counted as portable M0 checks.
The later retrieval/package milestones own those independent conformance and integration observations.
