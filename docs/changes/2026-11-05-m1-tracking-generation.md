# M1 Tracking Projection Incarnation Correction

The urgent engineer request cites hosted run 41994272561 at d9f/a114: Linux native watch passed, but native_reconcile_replay_rename_duplicate_and_delete failed with Recovery on its first scan at tracking_native.rs:171/156.
The [storage owner](../specs/storage.md#observation-journal-and-rebuildable-state) makes generated projections disposable and visible journal evidence authoritative.
This correction changes only the tracking projection, its journal-generation query, and its regression; it does not reopen the seven closed corrections or edit native.rs, Core, Root, Git, or policy bodies.

## Confirmed Mechanism And Evidence Limit

The old generated namespace contains only native root device/inode, backend, and writer.
A different visible stream at the same native root namespace can therefore select an earlier incarnation's generated frames.
Lookup then verifies an old cached offset against the new stream and returns Recovery although the current visible stream and source metadata are valid.
Fast native inode reuse between disposable hosted fixtures is a plausible trigger; the hosted artifact has no inode/cache telemetry to prove that trigger uniquely.
The public-operation regression retains an actual old journal outside the selected home, leaves its generated projection intact, and opens a fresh visible stream at the same native root.
OLD reproduces the exact Recovery from Fixture::scan; NEW completes, refuses the old cache's asset identity, preserves the archived bytes, and preserves the new visible identity on another scan.
This is not a claim that hosted CI has been rerun or accepted.

## Correction

Qualify the existing generated namespace by the bounded validated first visible record's exact-byte hash.
An empty stream gets a fresh disposable generation rather than a reusable empty-history key.
Replay and cached-frame proof verification remain the authority checks; the generation hash supplies only cache correlation.
One bounded first-record read per writer adds no whole-corpus size or writer-count ceiling, no second registry, and no generated-state deletion.
Partial, unavailable, and unknown metadata do not become journal corruption; no blanket Recovery remapping or test weakening is used.
Actual malformed/truncated journal evidence still requires recovery and remains untouched.

## Gates And Outstanding Work

The retained OLD regression log is ../gates/critical-projection-old.log, exit 101, with Recovery at tracking_native.rs:156.
The corrected nine-case tracking suite passes, including the original hosted-failing case, partial/unavailable refusal, malformed-tail retention, writer transition, replay predecessor, and the new incarnation regression.
With Rust 1.99.0, jobs=2, locked/offline dependencies, and invocation-private canonical target/TMPDIR, cargo test --no-fail-fast -p knowledge-storage reports 30 passed, 1 failed, 0 ignored, including all nine tracking cases and the compiled Scope consumer.
The unchanged positive Linux watch remains RED at native_apply.rs:651 with Exhausted; the actual owned empty-directory diagnostic reports inotify_init1 success and inotify_add_watch errno 28, with its FD closed and fixture removed.
Strict all-target knowledge-storage Clippy, warning-denied docs, workspace format check, and diff checks pass; exact commands and raw outcomes remain in ../gates/critical-*.log outside Git.
The production delta is 30 added and 4 removed lines; the regression adds 34 lines and does not alter any existing assertion.
Native hosted confirmation and independent consequential acceptance remain outstanding; this local pass does not waive them or the separately assigned Git failures.
The required FSEvents implementation remains preserved separately in ../gates/fsevents-pending.patch and ../pending-native while this urgent correction lands.
Linux inotify capacity remains an environment observation, not a reason to weaken or skip its positive watch check.
