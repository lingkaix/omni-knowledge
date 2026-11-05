# Composed Processing Admission Repair

This restores the accepted [resource owner](../specs/storage.md#effective-resource-accounting) and [processing owner](../specs/processing.md) after the source review of `9a807f984c9f7c41b75a16e59679de23a6892dab` identified A1–A4.
The review was source-only; this repair adds real local regressions and controls, without claiming a native Mac or production ASR qualification.

## Mechanisms

A1 uses Core P1's existing `split_claim` to consume real execution credit and retain job attribution, with no second reservation, queue, pool, or manufactured source digest.
Complete concurrent input/result/transport/child/storage needs are checked before processing effects; impossible whole needs are typed refusals rather than waiting on individual components.
Pre-effect contention rolls back partial components; failed claim transfers are truthfully released globally, not silently refunded to execution credit.
No new Core API or source edit is required.

A2 prepays bounded descriptor buffers and actual text/typed-result allocations; input and transport guards outlive their buffers.
Growing-inode reads use bounded `pread`, not metadata followed by unbounded `read_to_end`.
Transcript Value trees and preview rendering run in the real watched worker, while returned capacities retain actual Core holdings until consumers drop them.
The caller still owns native authority and before/after source version verification; FD-unlink proof does not prove a current version.

A3 binds generated file descriptors to the selected cache device and installs a prepaid native regular-file extent ceiling before child exec.
FFmpeg's actual MP4/WAV writes, whisper's private basename output, and the one EPUB staging inode cannot consume an after-the-write promise.
Actual deltas are reconciled once after collection, including known failed partial output; pipe diagnostics need no unreserved temporary files.
The source-clock clip ranges, codec recipe, transcript timestamps, installed-tool selection, and no-clobber publication remain unchanged.

A4 unifies parser, media, and textutil supervision in one bounded nonblocking I/O/RSS/cancellation/deadline loop with no detached writer or blocking wait/join.
Native group liveness and pipe EOF are established before reaping the pinned leader PID; exit/exec observation races are retried only within finite sampling/collection bounds, never accepted as zero cost.
Unresolved actual children, FD pins, pipes, artifact paths, and costs stay owned by `UnsettledChild`; failed native write reconciliation retains the actual file and credit in `UnsettledWrite`.
Lost unresolved child errors strand actual descriptor pins and Core credit rather than retargeting a live child's parent-FD argument.

## Verification And Integration

The two legacy public-boundary regressions fail on archived byte-identical `9a807f9` production: a complete impossible need returns WaitingMemory, and a one-byte storage promise permits an actual 48-byte FFmpeg output.
A real direct-exit/background-child regression also fails there; the test stops only its exact owned fixture PID before reporting RED.
The new implementation passes those regressions plus funded plain/HTML claims with zero global availability, native retained-capacity/source-cursor controls, actual EPUB generation/round-trip, bounded growing-FD read, blocked stdin, late silent RSS growth, output growth, and unresolved escaped-I/O recovery.
The original 14 unit, 7 adapter, 1 unstarted, and Root 11 extraction / 13 embedding / 3 runtime assertions are retained; there are no new ignored or empty helper tests.
Test setup serializes unit fixtures' process observations so unrelated runtime launcher sleeps cannot contaminate the existing owned-child assertions; no runtime production behavior or oracle was weakened.
Raw REDs, failed builder gates, exact commands, input hashes, and final gates are retained outside the clone in `evidence/omni/m3-admission-fix-artifacts`.

The [compiled API guide](../../crates/knowledge-processing/README.md) describes additive funded entry points and the explicit low-level collection return change.
Only Processing, three Root re-export shims, this record, and the required existing-version SHA-256 test dependency edge are changed.
Root library/main/MCP/kernel and Core remain under their existing owners; their actual funded-call migration, source authority, error projection, and native Mac qualification remain integration obligations, not claims of this repair.
