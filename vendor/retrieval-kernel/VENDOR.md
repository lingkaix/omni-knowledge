# Vendored Retrieval Kernel

This guide owns provenance and the local patch ledger.
It does not claim that the mount, archive, or large-library interfaces are implemented.
The [README](README.md) retains the upstream overview and adds the native adapter guide. Its original/current hashes and replay are recorded below; it is not Omni's platform qualification.

## Source And Licenses

The source is `packages/retrieval-kernel` from public [OpenKit revision b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1](https://github.com/lingkaix/openkit/tree/b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1/packages/retrieval-kernel).
[PROVENANCE.json](PROVENANCE.json) records every original crate file's SHA-256 and the retained file's SHA-256.
All original files remain, including Rust tests, upstream notice tooling, supplied notices, and the IPADIC archive.
[LICENSE](LICENSE) is the exact upstream root Apache-2.0 license.
No root NOTICE file exists at this revision.
Third-party licenses remain separate; retain [notices](notices/README.md), its provenance inventories, and [IPADIC-COPYING](vendor/IPADIC-COPYING) when redistributing.
The retained notices are upstream evidence, not a complete audit of a newly resolved downstream dependency graph.

The dictionary archive is `mecab-ipadic-2.7.0-20250920.tar.gz` from `https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz`.
Its SHA-256 is `a7ba9f645ffe7094e56ae1c4a81d100df8fbb1e28bbe1792622e9728e162db3d`.
The [probe guide](../../tools/kernel-probe/README.md) verifies it before seeding a dedicated Lindera build cache.
Cargo offline mode alone does not prevent a cold dictionary build script from contacting the network.
No dictionary or model download is required by the seeded probe.

## Applied Patches

Apply the historical [M0 patch](../../tools/kernel-probe/patches/m0-vendoring.patch), the [native adapter overlay](../../tools/kernel-probe/patches/m3-native-owned.patch), the [STORED directory overlay](../../tools/kernel-probe/patches/m4-stored-directory.patch), then the [native resource observer overlay](../../tools/kernel-probe/patches/native-resource-observer.patch), to the exact pinned upstream crate. The ordered patches replay all adaptations below, including the two new local source/test files. Original hashes stay unchanged; current hashes cover all52 source entries. Metadata and the patch ledger are not a new license or platform proof.
M0-P01 and M0-P02 change no analyzer, scoring, coverage, source admission, generation publication, or test assertion.

| Patch | Necessity and effect | Future OpenKit candidate |
| --- | --- | --- |
| M0-P01: machinery lock location | `src/store.rs` now includes `../../upstream/openkit-b67cd0f2.Cargo.lock` and explains its provenance role. The old relative include would silently hash Omni's root lock after relocation. The retained bytes preserve upstream machinery identity. | Generic explicit build/dependency identity input instead of a monorepo-relative assumption; do not upstream this host-specific filename. |
| M0-P02: Rust 1.99 Clippy compatibility | `decode_digest` uses fixed two-byte `as_chunks` instead of `chunks_exact`. Its prior 64-byte validation makes the empty remainder invariant unchanged. Rust 1.99's strict Clippy gate rejected the original form. | Direct mechanical iterator update, with unchanged decoding and public digest-window tests. |
| DIAG-01: native limit failure context | `set_limit` reports the resource, errno, inherited limits, and requested limits from the failing `getrlimit` or `setrlimit`. | Diagnostic context at that call. Do not upstream a changed ceiling, a fallback, or a resource-policy change. |
| DIAG-02: self virtual and resident sizes | One summary reports this process's virtual size and resident size at the same install. Linux copies `VmSize` and `VmRSS` from `/proc/self/status`. macOS reads `mach_task_basic_info` `virtual_size` and `resident_size` in bytes. Success writes one stderr line. Failure puts that one summary in the existing error. | Fact at the install call. The AS 2 GiB, file 512 MiB, and CPU order stay unchanged. The numbers are not a ceiling. |

Remove a patch when upstream supplies its responsibility, then rerun the same downstream and parity checks before updating provenance.
The [change record](../../docs/changes/2026-11-04-kernel-vendor.md) records M0 proof and exclusions.

## Observation Corrections

OBS-01 and OBS-02 are entries in the same replayable patch as M0-P01, M0-P02, DIAG-01, and DIAG-02.
They do not change production path handling, resource ceilings, or controller policy.
`PROVENANCE.json` keeps every original upstream hash and records the current vendored hashes of `src/main.rs` and `src/tests.rs`.

| Correction | Necessity and effect | Boundary |
| --- | --- | --- |
| OBS-01: canonical directory-race target | The fixture fault path now uses the canonical root that `Directory::new` binds. A `TMPDIR` symlink alias otherwise misses the pre-stat and pre-open removal. | Fixture setup only. Coverage, hit, unavailable-count, and privacy-marker assertions stay unchanged. No production path normalization. |
| OBS-02: `ru_maxrss` unit | Darwin bytes come from XNU copying `resident_size_max`, which Mach `task_info` defines in bytes. Linux `getrusage(2)` reports kibibytes. The observer converts to bytes and keeps the 512 MiB ceiling. | The 48 MiB source bound, allocator check, exact-query checks, and child identity stay unchanged. This is not a budget increase. Native macOS execution remains unobserved. |

## M4 Native Invocation Seams

M4-P01 adds an explicit consuming-build fingerprint to actual kernel machinery checks at publication and reader open.
The prototype `Kernel::new` retains its historical upstream-only identity; product callers use `Kernel::with_build_identity` through `knowledge-retrieval`.
M4-P02 adds shared caller cancellation/deadline checks to actual publication/query operations without raising the existing per-shard or per-format ceilings.
M4-P03 exposes a real native generation reader pin; its filesystem reader lock remains live until the pin ends.
These are generic future upstream candidates and import no Omni authority, library, service or model types.
The replay and current source hashes include these patches; all original upstream hashes, licenses, locks, diagnostic numbers and AS/EINVAL evidence remain unchanged.
The facade and its guide state the still-required source/Directory/streaming-orchestration boundaries; these seams alone do not certify the final product bound.

M4-P04 supplies a live caller guard in `Control`; actual work checks its native current policy as well as deadline/cancellation. The generic library seam imports no authority types.
M4-P05 is an Omni-local standalone-executable host adapter, **not an OpenKit library candidate**. The binary uses the actual Core account, admits parent/worker costs, launches one exact owned pure-kernel process group, observes native full RSS and `wait4` peak RSS (Linux KiB converted to bytes; Darwin bytes), and collects on cancellation/error before releasing resources. Native parent identity and parent-death/watch mechanisms reject a forged internal role. A bounded 100 ms missing-sample window retains the full promise and uses real exit peak evidence, never a fabricated zero sample. The private account directory is created on the selected store volume without changing existing directory permissions.
M4-P06 replaces the old executable AS oracle with preserved-inheritance plus real owned-worker RSS assertions. No `RLIMIT_AS` ceiling is installed on either OS; the historical 2 GiB/EINVAL receipts and DIAG rows above remain historical facts, not current enforcement. Existing CPU/file ceilings and the 48 MiB/512 MiB library stress oracles remain. The live one-byte worker-promise test requires a numeric real-RSS refusal; Linux execution does not certify native macOS.
`PROVENANCE.json` records original files unchanged in scope and separately lists new local source/test hashes. The complete upstream-to-current replay includes additions. The owned pure kernel creates no descendants; this proof does not close the separate product processing descendant-executor review.

M4-P07 adds operation-only scan-field selection to the same exact/regex scanner: body-only for generic text, or body plus display name/title/description for ordinary Root navigation. Tags/digests/private identity metadata cannot become pseudo-body grep matches. Eligibility, current-document identity, complete scanning and the shared analyzer/ranking implementation are unchanged.
M4-P08 repairs consequential native settlement: only exact successful wait4 sets reaped; bounded stop/poll keeps the full child/native resource capability quarantined on unknown collection, and ECHILD refuses signalling a stale PID. Owned reads/writes have nonblocking monotonic deadlines, the supervisor bounds its pure-worker lifetime, and diagnostics cannot block collection. Normal owned pipe-backpressure and unknown-reaper controls run in addition to the live RSS/inherited-AS tests. This is not native macOS qualification, a new executor, or product durable recovery.

M4-P09 extends the same native query implementation with immutable generic byte directories and `SourceGeneration`. M5 supplies verified held archive ranges; actual Tantivy postings and canonical literal/regex readers query those ranges without extraction or a second engine. Query/refill currentness and the complete source/reader/cost lifetime remain with the real caller. The consuming adapter now passes its actual source/Claim guard through initial M5 verification as well. Streamed publication, full-corpus orchestration and native vector/model-space work remain separate required work; this overlay is not full activation or platform proof.

M4-P10 retains every executable resource condition while replacing the Linux-only macOS test instrument. The actual controller reports bounded startup `getrlimit` readback on its existing stderr diagnostic channel (no protocol opcode/field). Both OS tests require inherited AS unchanged, actual file ceiling536870912 and CPU65; Linux additionally retains its original `/proc` assertions. Darwin samples the actual child graph and before/after BSD identity plus resident task bytes using libproc; exactly one actual worker and positive actual RSS are still required. The Apple `proc_listchildpids` wrapper returns a PID count, NOT bytes. A failed assertion keeps the actual controller in foreground through EOF/owned-worker drain/wait. No ceiling, source/analyzer/scoring semantics, data protocol or fake backend changes. This instrumentation/source replay is NOT native macOS acceptance until actually executed there.

## Dependency Identity Binding

The retained [upstream Cargo.lock](../upstream/openkit-b67cd0f2.Cargo.lock) has SHA-256 `88be6b21f6430d1aa5bf3b730822905e37ed7514a2378a49b454265797a0947a`.
`machinery()` still binds this exact upstream evidence and the existing dictionary/schema/settings inputs.
The historical prototype constructor does not automatically bind the actual consuming workspace's resolved dependencies.
The explicit M4 constructor incorporates the caller's consuming fingerprint into the same manifest compatibility checks.
The Git and path probes retain their actual Cargo-generated locks separately.
The standalone crate lock records the M0 vendor test build, not Omni's future production resolution.

Before production generation reuse, the retrieval integration must bind the actual Omni root Cargo.lock digest, resolved feature/target identity, vendor patch/source identity, and dictionary identity into the generation compatibility check.
Use an explicit generic identity parameter or build-supplied fingerprint; do not discover a parent lockfile from the current working directory.
Use the same identity at build, publication, open, and query; a mismatch requires a derived-generation rebuild.
An outer identity check must reject incompatible generations before kernel access if the kernel does not yet accept this parameter.
The workspace owner supplies the real application lock and the retrieval owner supplies this integration; M0 does not change either owner's production code.

## Required Seams: Characterized, Not Implemented

These are generic future upstream candidates under the accepted retrieval design.
They must not introduce Omni library, GUI, package, or OpenKit authorization concepts into the kernel.

| Seam | Actual obstruction at the pinned source | Smallest required direction and proof |
| --- | --- | --- |
| Admitted byte reader | `Kernel.source` is a filesystem `File`; private `open_source` uses descriptor-relative paths. Private `TextFile` has only file and inline forms. Publication and query both reopen these sources. Public file ranges and inline text do not inject a multi-mount or immutable archive reader. | Accept an admitted bounded read/seek source provider with explicit unit identity and lifetime. Preserve complete-unit digest verification, declared ranges, revalidation and stale withholding. Compose actual mount/archive readers with exact/regex and lexical publication. |
| Read-only generation and Tantivy Directory | Private `Pin` contains a filesystem path, manifest, and reader lock. Pinning opens CURRENT, LOCK, READERS and manifest files. Query and no-op validation call `Index::open_in_dir`; sidecars open filesystem paths. Merely replacing the query's index-open call is insufficient. | Accept an immutable generation provider that supplies a Tantivy Directory, bounded manifest/sidecar reads, identity and a lifetime pin. Keep writable publication separate and use external reader state when appropriate. Prove STORED archive querying with extraction denied, digest verification, container replacement and pin retirement. |
| Caller budgets | Public publish/search wrappers construct private `Budget::new()`. Limits include 60 seconds, 10,000 objects, 8 MiB inventory/frame, 512 MiB temporary index, 32 MiB writer, 1,000 hits and 64 KiB regex lines. | Expose generic validated caller budgets/deadline/cancellation without weakening safe bounds. Preserve stopped-work coverage and prior-generation publication on failure. Do not solve scale by an unbounded cap increase. |
| Inventory streaming and deterministic shard planning | `ScopeView.documents` and directory inventory materialize a full vector. `validate_view` serializes the inventory under a fixed frame ceiling. A larger source inventory cannot be admitted as one scope. | Plan stable bounded shards from source keys and encoded sizes, report oversized units, scan all admitted shards, and preserve deterministic occurrences. Fuse ranked lists rather than comparing independent raw BM25 scores. Prove boundary sizes and stopped/incomplete plans. |
| Consuming dependency identity | Machinery reads only the upstream monorepo lock and hard-coded dictionary/settings identity. | Accept a generic dependency/build fingerprint with mismatch/rebuild proof as described above. |

No M4 seam trait or alternative production engine is introduced at M0.
The Git probe is historical feasibility evidence in an isolated workspace; the path probe is the selected vendor consumer.
The original M0 dependency-only landing did not replace application search. The subsequent rootlib integration now publishes/queries the actual kernel through `knowledge-retrieval`, with bounded deterministic legacy-snapshot shards and file-deduplicated rank-list fusion. Its old lexical/trigram construction is retired. This does not implement the admitted arbitrary-byte reader, archive Directory or streaming orchestration interfaces above, and does not assert final acceptance.
