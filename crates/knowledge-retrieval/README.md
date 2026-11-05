# Admitted Kernel Adapter

This crate owns the native admitted invocation seam, not orchestration or a second search engine.
[Retrieval semantics](../../docs/specs/search.md) owns lexical behavior and generation composition.
[Storage accounting](../../docs/specs/storage.md#effective-resource-accounting) owns the one process resource account.
The actual implementation calls the vendored `Kernel`; no fake backend, alternate analyzer, or local resource pool is present.

## Public API

- Startup establishes `knowledge_core::admission::Account` once; this crate uses only `Account::current()`.
- `Work::reserve(Reservation, Origin)` reserves before allocation and preserves Core's `Decision::Waiting` versus whole-budget typed refusal.
- `Work::split_claim` transfers an actual queued claim's existing reservation into an independently retained child/runtime/model/pin lifetime without charging twice or retaining a mount execution slot.
- `Work::{remaining,grow,written}` inspect/acquire the same account's promise or confirm actual generated-byte deltas; this is not an RSS observation.
- `KernelAdapter::open(source, store, build_identity)` opens an actual native kernel and binds the consuming build identity.
- `KernelAdapter::publish(view, expected, &mut work, &control)` invokes actual kernel publication under the unchanged per-shard format ceilings.
- `KernelAdapter::search(query, &mut work, &control)` invokes actual lexical/exact/regex retrieval, including native reader locks and source verification.
- `KernelAdapter::pin(scope, work, &control)` consumes an independent work lifetime and returns a real `PinnedGeneration`; its native lock drops before the resource charge.
- `Control::new(deadline)` and `cancel()` share cancellation with actual in-flight kernel checks; cancellation is a request, not a rollback assertion.

The host reserves persistent dictionary/runtime/model memory for its actual process lifetime separately from each operation.
The conservative per-shard invocation requirements include caller inventory, native readers, writer and encoding space, not a measured physical-memory guarantee.
The caller must acquire the inventory reservation before materializing that inventory.
`publish_requirement(encoded_bytes)` uses checked arithmetic; it does not introduce a whole-corpus ceiling.
Inventories larger than a kernel shard need the orchestration owner's streaming deterministic partition planner, not a larger hostile-format limit.
The caller must retain `Work` while its synchronous resources exist and split all detached/retained lifetimes before ending execution.
Losing the underlying Core holding cannot be repaired by a force-release registry.

The build identity must cover the consuming Cargo.lock, resolved features/target, current vendor source/patch identity and dictionary identity.
The adapter passes it into actual kernel machinery checks at build/publication/open/query; changing it requires a rebuild before reuse.
It is not a source authorization token.

## Necessity And Ablation

Without `Work`, adapters could silently bypass the sole process account or release live reader memory with an execution slot.
Without native kernel calls, a DTO wrapper would not replace competing lexical behavior.
Without the consuming identity, upstream lock provenance alone would falsely certify downstream derived-generation compatibility.
Without shared `Control`, cancellation would only change a status label while kernel work continued.
No alternate scheduler, resource pool, persistent operation registry, or lexical implementation is needed for this seam.

## Integration Limits

The root `KnowledgeBase` now calls this adapter for actual lexical/grep publication and search. Its retained snapshot is projected into deterministic bounded shards; independent lexical lists use reciprocal-rank fusion, not raw cross-shard BM25 scores. The old publisher writes a kernel reference, not a competing Tantivy or trigram index. Native CURRENT corruption is a query failure, never an old-engine fallback.

Rootlib starts/reuses the actual Core owner, verifies source digests through descriptor-relative opens, charges retained snapshots through cache/reader lifetimes, and binds M3's actual admitted file APIs. Deep-copy handoff admits the new buffers before copying and drops the original M3 buffers before finishing their holdings; this avoids one permanent holding record per corpus source without aliasing or early release. Raw compatibility results without a parent lifetime keep M3's documented conservative process charge.

The standalone kernel executable has a native owned pure-kernel child watchdog, complete resident/peak-RSS observations and typed failures. It does not install RLIMIT_AS on either OS. The kernel worker launches no child processes; this proof is not the product processing descendant-executor proof. Linux is exercised; macOS native execution remains unobserved.

## Generic Native Generations And STORED Queries

`retrieval_kernel::ByteDirectory` supplies owned native range handles with current caller checks, not a path-backed emulator or an authorization DTO.
`SourceGeneration::open` validates the existing native manifest digest, consuming machinery, bounded inventory and representation coordinates.
Its `search` method and the ordinary filesystem kernel enter the same query implementation, including analyzer/scoring, title boosts, CJK candidates, source verification, canonical literal/regex scanners and original locators.
A native provider must retain its actual generation, source, current policy, descriptor and concurrent resource costs through all handles.
Only the immutable metadata lock retains that real provider lease; writer and unknown locks refuse, and immutable readers use manual reload without a watcher thread.
The unchanged 8 MiB manifest, 10,000-document shard and 512 MiB native-generation ceilings remain hostile-format bounds, not full-corpus claims.
These narrow vendor changes are upstream seam candidates; the pinned upstream source, lock and dictionary provenance in [VENDOR.md](../../vendor/retrieval-kernel/VENDOR.md) remain unchanged.

`StoredIdentity::new(actual_consuming_build)` returns the actual kernel machinery, Tantivy/schema, processor, dictionary and exact-segment format facts.
`requirements(selected_generation)` supplies M5's `IndexRequirements` for a lexical-only tuple; it never reads producer trust flags as native requirements.
`StoredGeneration::open(source, requirements, scope, consuming_build, prefix, plan, work, bounds, cancel, control)` consumes an independent actual `Work`, normally from `Work::split_claim` on the existing queued job.
The source implements `ArchiveSource::{check,file}` on the host's real `NativeSource`/permit carrier.
Its checks must retain the actual selected policy, namespace, source version and source costs, not merely compare a path or digest.
The supplied `Control::guarded` must also check the real Claim's complete key and stop request; a cooperative `Control` is not itself permission.
Root owns that implementation and its dispatcher/delivery integration; this crate does not edit Root or claim activation.

`StoredPlan::requirement` prepays the concurrent opaque native-reader, manifest decode, range table, index buffer/cache and result envelope before opening native objects.
The process dictionary/runtime remains charged once by `KernelAdapter::initialize_runtime` on the same account.
M5 verification borrows that SAME already paid holding with `Work::guarded_reader`; its bounded scratch counter never makes a second late reservation or refunds the actual parent holding. Insufficient prepaid capacity refuses before allocation. The read-only seam cannot charge generated storage.
The adapter consumes M5's Result-returning `index_paths`, `with_indexes`, `FrozenArchive` and `StoredRange::extent` APIs.
It verifies all inventoried entry digests through M5, copies only bounded names/extents, and retains the actual source owner rather than letting a borrowed package reader escape its callback.
Native components can open only complete inventoried index members; canonical source ranges can open only verified STORED members of the same held archive.
After M5 verification, every native range refill checks the native lease, complete held-file version and shared control, using checked ZIP64 offsets without mmap-alignment assumptions.
Initial M5 framing, payload verification, scratch admission and later range refills check the same actual source owner and Control through `Work::guarded_reader`. ZIP positional reads are bounded by the existing scratch window and checked both before and after each native read. Real Root activation must bind its current source/permit/Claim, not a DTO-derived callback.
The deciding native-owner control revokes an actual service selection during initial verification, before a later corrupted payload can complete. Disabling only that guard returns Checksum rather than currentness refusal. A separate control exhausts free global credit after the complete real Claim plan was paid: prepaid reading succeeds; reverting to late admission returns Waiting. These are local builder controls, not final host/RSS acceptance.
There is no extraction, scratch index, rebuild, second lexical invocation, writable fake filesystem, or semantic fallback.

`StoredGeneration::search` consumes its noncloneable reader and returns `StoredResult` with a borrowed `result()` and a current-source `check()`.
One prepaid envelope therefore permits only one active query/result; cloning a reader cannot create concurrent spending credit.
`StoredResult::into_generation` destroys every prior result buffer before returning that same owned envelope for another query.
A compile-fail public-interface check protects the noncloneable reader boundary.
The actual source, descriptor, native generation and independently retained `Work` survive with the result after the caller's original reader binding is consumed.
Keep that owner and all paid encoding/copy buffers through actual delivery/write settlement; check before each controllable output chunk.
A caller must not clone raw result bytes into an uncharged or detached lifetime.
Dropping synchronous settled owners releases their own cost; it is not settlement of unknown children or an implicit queue completion.

The builder-owned `stored_generation` integration test publishes through the existing native writer, packages its actual components, verifies them through public M5 APIs, deletes the loose source and native store, and queries real archived Tantivy postings plus canonical literal/regex ranges.
It checks English/Chinese/Japanese retrieval, exact original byte locators, real service permits/current-parent replacement/policy revocation, real Claim cancellation, retained result costs, typed whole-budget refusal and contention.
The external fixture producer adds its real native tuple to a public M5-captured inventory because M5 `capture` still emits omitted index coverage; that producer gap is not represented as completed package capture support.
Existing kernel, package and orchestration oracles remain unchanged.

## Historical Producer Qualification Checks

The prepared base is `89b878e587cec507139a7f9bf1722d802090f4bd`; the handoff log identifies the resulting committed slice.
All checks use offline locked Cargo, two build jobs, a private disk-backed target/TMPDIR and the verified pinned dictionary supplier.
`cargo test --offline --locked -p knowledge-retrieval -p knowledge-search -p retrieval-kernel -p knowledge-package` passes, including existing untouched oracles and the new owned archive composition.
`cargo clippy --workspace --all-targets --offline --locked --no-default-features -- -D warnings`, `cargo fmt --all -- --check` and `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --offline --locked --no-default-features` pass.
`cargo test --workspace --offline --locked --no-default-features --no-fail-fast` fails in eight Core/Root targets: the two Core cache consumers, Root lib, extraction, knowledge_base, search_cache, status and unified_search.
An exact Git-archive copy of the prepared base also fails those eight targets: the Core nested consumer lock refuses, Root's aggregate-accounting assertion underflows, and parallel extraction/index fixtures encounter admission waits or native file/lock conflicts.
Individual parallel failing-case sets differ, so this is evidence of inherited target/failure classes, not per-case equivalence or a passing whole-workspace gate.
Those bodies and their checks remain unchanged and belong to their existing owners; no failing oracle was weakened or skipped.
The prepared root lock lacks M5's package record and its pinned canonicalizer dependency closure; minimal offline resolution adds `knowledge-package 0.1.0`, `serde_json_canonicalizer 0.4.1` and `ryu-js 1.0.3`, with zero existing IDs changed or removed.
This differs from the brief's zero-new-ID premise; no full lock regeneration or unrelated yanked-version update occurred.
The public API and local composition are compiled builder evidence, not independent review, whole opaque RSS qualification, native Mac evidence or activation approval.

## Required Follow-Up And Activation Hold

This slice is a compiled generic native-generation/query and STORED-range mechanism, not full M4 completion.
Generic streamed publication/ingestion, full-corpus bounded partition orchestration, funded immutable paged disk-vector build/open/query/pin/retirement with complete model-space identity, combined tuple publication/recovery, and final product generation/storage/history composition remain required.
The old `VectorStore` and Root snapshot/vector bridge are not qualified substitutes, and this lexical-only helper does not certify combined model-space compatibility.
M3 repaired the ordinary Root CLI owned-error exit path; the separate held M6 activation/foreground-drain obligations remain. Logical reservations are not measured whole opaque RSS, child-settlement or native Mac evidence.
The per-refill M5 guard is now wired and locally exercised. Real Root caller glue, complete-index producer integration, actual native macOS execution and product activation remain required. Work continues solo; this guide does not imply a new reviewer or delegated owner.
No final M4/M6 or B3/B4 acceptance is implied.
