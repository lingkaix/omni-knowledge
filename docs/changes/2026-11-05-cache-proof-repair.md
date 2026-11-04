# Knowledge-Core Cache Proof Repair

This record owns the independent corrections to the public cache contract tests after review.
It does not accept the cache, implement a production repair, or add a fixture registry.
The tests remain `crates/knowledge-core/tests/cache_contract.rs`, `crates/knowledge-core/tests/cache_owner.rs`, and the support modules under `crates/knowledge-core/tests/cache/`.
The auxiliary consumer is `crates/knowledge-core/tests/cache/consumer/`, a path-dependent package with its own manifest, lock, and private target.
Production lines in this change: 0. The lease forbids production edits, so the test-to-production ratio is not a reason to add implementation.

## Reclassification

`cache-tests/author-logs/contract-owner-3.log`, SHA-256 `4a22b3a815df3d18cf22c2470ecbe9916e8555d69c40634b4e2e79c24b29e05d`, is retained.
That log shows `unknown_entries_symlinks_and_failed_candidates_are_retained` failed with `Conflict` at the then-current `cache_contract.rs:648` (14 passed, 1 failed).
That failure is the same-key candidate collision after reopen. It is not an observer defect.
The new test `reopened_same_processor_reaches_duplicate_and_cancellation` isolates that case.
The unrelated new-key cancellation inside the retention test stays.
On base `653fd0ef088f255858dd30cbc3df1130c37a8cb5` the new test is an honest failure: the same source and processor, after every pin and the owner are dropped and the cache is reopened, must return `AlreadyPresent` for the duplicate and `Cancelled` for the later cancellation.
A `Conflict` there is the retained red. Keys and expected values are not changed to fit a later production candidate.
The historical [cache contract test record](2026-11-04-cache-contract-tests.md) is not rewritten.

## What Changed

Same-process reopen uses the original source and processor. Duplicate publication must be `AlreadyPresent`. Cancellation of a later publish on that same binding must be `Cancelled`, input-free, and must leave the installed bytes readable. The test does not switch the processor or the key to avoid `Conflict`.

Corruption captures the admitted mode, flips one payload byte, and restores that mode before the fresh pin. Complete objects stay mode `0400`. `pin` opens and verifies the complete object, so a `Corrupt` refusal grants no reader and no bytes. When a reader is granted, a range that excludes the flipped byte must still be `Corrupt`, because `read` verifies the entire payload. Republish does not repair the bytes. A second fresh refusal stays `Corrupt`.

The entry scan uses `max_entries = 8`, which is a finite ceiling the public limits admit, and twelve real extra entries. The refusal is `Budget`. The directory snapshot is unchanged. An absent prohibited root is an otherwise valid root list plus one absolute missing path. It is not the dangling symlink from the earlier arm.

Processor admission compares structured `ProcessorIdentity` values. It does not compare a fingerprint with itself. Public fingerprint checks remain separate. A dependency correction changes only the named transcript digest or only the named annotation digest.

Fixture digests use the already locked `sha2` 0.10.9 inside the auxiliary consumer. The integration tests do not carry a second SHA-256. Published empty, `abc`, and 56-byte vectors stay.

The helper is a Cargo package bound to this knowledge-core crate. Its build uses a private target, two jobs, no inherited jobserver flags, and `--cfg cache_public_consumer`. A lock serializes that build. Dep-info must name `src/cache.rs` and `src/cache/native.rs`. The identity file records `HEAD`, the executable path, and the executable digest. A production-only source change rebuilds the helper without a test-file edit.

The observer classifies a missing terminal separately from a bounded timeout. It admits pass, deliberate failure, early end of output, a real hang, a line followed by a hang, and cleanup when the assertion unwinds. Every terminal wait is bounded. Cleanup kills only a child whose `/proc` start token still matches, and the reap result is separate from the product result. The post-fork case observes that the child has left userspace, keeps the competitor `Busy` while the parent holds the cache, reaps only after that observation, and releases only after the reap. The forked child arms parent-death and a drop guard. This is not a process manager.

## Gaps That Are Not Cache Amendments

No test deletes a cache object. Garbage collection, eviction, a job queue, a root detector, service authentication, and full storage integration stay absent.
The same-key reopen failure on `653fd0ef` is evidence about the unchanged production candidate. This record does not repair it.
Source-only integrity and owner negative arms stay with the producer and run against these oracles unchanged.
The auxiliary package is one public consumer. It is not a registry and not a protocol framework.

## Checks

Rust 1.99.0 and Cargo 1.99.0, via `rustup run 1.99.0`. Commands use `--locked --offline`, two Cargo jobs, and a private target directory and `TMPDIR`.
The knowledge-core package does not compile the vendored kernel.
On base `653fd0ef088f255858dd30cbc3df1130c37a8cb5`, after the committed harness text, `cargo test -p knowledge-core --test cache_contract --test cache_owner --test manifest_codec --locked --offline -j 2 --no-fail-fast -- --test-threads=8` exited 101. The process started at 2026-11-05T07:50:59+11:00 as pid 3300646 and has exited.
`cache_contract` reported 15 passed and 1 failed. The failure is `reopened_same_processor_reaches_duplicate_and_cancellation` with `Conflict` at the same-key duplicate assertion. Nothing was ignored.
`cache_owner` reported 3 passed, including pass, deliberate failure, early end of output, hang, line-then-hang, and unwind cleanup. `manifest_codec` reported 19 passed in that same invocation; its stimulus correction is a separate commit and is not part of this record.
`cargo clippy -p knowledge-core --all-targets --locked --offline -j 2 -- -D warnings` exited 0. The auxiliary consumer package clippy, with `--cfg cache_public_consumer`, also exited 0.
An earlier invocation without `--no-fail-fast` stopped after `cache_contract` (14 passed, 2 failed) because `pin` on the corrupted object returned `Corrupt` before a reader existed. That observation is retained. The committed oracle accepts that complete-object refusal and still requires an excluding-range `Corrupt` read when a reader is granted.
The auxiliary consumer identity written by that final run records `HEAD` `653fd0ef088f255858dd30cbc3df1130c37a8cb5` and executable SHA-256 `76f2799858e15e433fdbe4c9877ea9e8cbc16cf335f4991b96f97546448c8dea`, and its dep-info names `src/cache.rs` and `src/cache/native.rs`. An earlier build of the same tests recorded executable SHA-256 `6edf530cc75e537845d979cc8895267c44eb6f489b4ddfd5b91d47ecba98aee2` before clippy rebuilt the helper.
Exact command text, exits, and file hashes are in the cache proof-repair evidence log.
