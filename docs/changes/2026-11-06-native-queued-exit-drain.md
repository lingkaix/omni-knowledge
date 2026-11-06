# Drain actual queued output after an owned native exit

Actual public2ed run42222129938: Linux698 succeeds; macOS660 passed, 2 failed, 1 declared ASR opt-in. Both failures expose the same recorded native branch: `pipe-group-signal`, PermissionDenied/errno1, after exact owned leader exit and native group sample with zero live members. History indeterminate-ref recovery and Storage interrupted-install recovery fail with Io/History(Io); this does not retrospectively diagnose580's untraced Io. Strict was not reached, so the prior cfg-only import fix is not yet native-qualified.

## Necessary ablation

No live group member needs signalling merely because stdout/stderr have queued bytes or have not yet reached EOF. Remove that eager empty/zombie-group signal; retain the SAME foreground child/pipe/credit owner, bounded nonblocking drain, deadline/currentness checks, actual group sampling, both EOFs and final native collection/reap. Live group signalling and all unresolved-owner retention remain. No errno is ignored, effect retried, assertion skipped, or limit raised. Original64MiB History,512MiB file and65s CPU promises are unchanged.

## Deciding control

A real owned `/bin/sh` writes one byte to each pipe and exits. Test-only thread-local timing waits for its real unreaped exit and drains one real batch per tick, forcing queued bytes before EOF; a signal observer records actual native attempts against an actually exited/empty group. No Git/signing/OS backend is substituted.

OLD executes one actual case and fails one empty-group signal versus zero. NEW executes one actual case and passes, preserves both exact bytes/status, leaves no unresolved native owner and settles the actual Core claim. Existing History and Storage suites pass. Test timing resets on Drop; production read batching is unchanged. New changed-head full Linux/macOS gates remain required; this is not whole-product/native/M4–M6 READY.

The repair-only qualification tree excludes in-progress tuple publication/reader work. Its source is frozen independently; qualification does not authorize those separate mechanisms.

## Actual changed-head macOS follow-up

7bd run42230979358 Mac runtime662P0F1ASR opt-in; both formerly failing native History/Storage controls pass. Its strict step fails because the test-only QueuedExitFixture wrapper is compiled on macOS while its existing native_tests consumer module is Linux-only. Match only the three wrapper cfgs to that consumer. No lint suppression, skipped existing case, production/runtime/ceiling change. The test timing hooks and actual production drain are unchanged. Next-head actual Mac strict remains required. Linux hosted job was still provisioning when this macOS evidence was collected.
