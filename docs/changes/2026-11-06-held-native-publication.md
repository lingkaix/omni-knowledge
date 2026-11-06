# Held Native Publication

## Necessity

The existing native reader already accepts `ByteDirectory` and bounded `RangeReader`.
The same kernel's Text/Representation publisher still opened a filesystem pathname,
then opened it again for final verification. An actual M3 representation held by a
native owner or M5 range could not reach that writer without a loose-file path bridge.
A new index engine, whole-corpus resident copy or hash-FD-then-reopen is unnecessary.

## Change

`Kernel::with_source_directory` supplies the SAME generic native input seam to bounded
publication. Each Text/Representation keeps one actual handle through verification,
indexing and final verification. Missing members never fall back to filesystem paths.
A new native input owner is verified even if prior identity labels match. Native input
failure preserves prior CURRENT rather than publishing a successful partial generation.
Legacy filesystem behavior, schema/analyzers/scoring and hostile-format ceilings stay.
Held generation queries continue to use the existing `SourceGeneration` reader.

`KernelAdapter::open_held` uses already-paid setup Work, not another late global reserve.
The host retains that setup until AFTER adapter/source objects die, initializes shared
runtime at startup, and separately prepays complete writer/storage/reader costs. Resource
Work is not source or destination authority. Actual native owners retain those separately.
No new account, queue, backend, archive parser, corpus cap or processor is introduced.

## Deciding Controls

`held_publication` first makes an actual kernel/M5 archive, then deletes its loose source
and seed index. An actual SDK Refresh/selected-root permit and ONE live Claim guard M5
verification/refills. All remaining global memory is held while the already-paid native
constructor and real publisher consume the held archive input. Both Text and Representation
reach actual Tantivy body postings with exactly one source open. The job remains unsettled.
Wrong verified member bytes, a missing member and an actual selected-directory move refuse;
prior CURRENT remains byte-identical. Actual synchronous owners die before loan rejoin.

Five independent ablations each execute `running 1 test` and fail: pathname input bypass
(open count0), member reopen after verification (count2), blind old-label reuse (wrong
actual member accepted), historical partial-native success (missing member accepted), and
late global setup admission (`Reservation` under pressure). Restored test6 passes. Initial
fragment-type, wire-budget spelling and field-name fixture failures remain recorded.
The five ordered vendor overlays replay all50 original and52 current source hashes exactly.

## Boundary

This is a bounded-shard IO seam, NOT full-corpus persistence/query, whole lexical/vector/
source/model tuple publication, startup activation, native macOS proof or full-product READY.
Actual Root's aggregate producer and immutable tuple orchestration remain required.
