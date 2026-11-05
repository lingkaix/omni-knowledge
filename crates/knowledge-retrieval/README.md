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

Still unimplemented here: admitted arbitrary-byte providers, read-only archive Directory composition, streaming large-corpus orchestration, disk vector orchestration, and final product generation/storage/history composition. The legacy snapshot bridge and deterministic bounded shard planner are not replacements for those owner interfaces. No final M4/M6 or B3/B4 acceptance is implied.
