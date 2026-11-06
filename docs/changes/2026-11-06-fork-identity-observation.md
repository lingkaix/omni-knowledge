# Fork Identity Observation

## Observed Failure And Cause

Hosted 83/42171141711 completed Linux 693 successfully. macOS ran 656 passing cases,
failed `cache_owner::post_fork_child_destruction_does_not_unlock_the_parent` at
`child start token`, and retained one explicit ASR opt-in. Strict Clippy did not run.
This is distinct from the previous watch Budget and large-enum failures.

The helper captured its child's real birth identity, printed CHILD, then immediately
released that child to destroy inherited state and exit. The external test independently
read the live native birth identity only after consuming CHILD. It could therefore reach
the Darwin observer after child exit. No source evidence establishes a cache-lock defect
from this failure. The existing exact-child NOWAIT exit/reap proof remains necessary.

## Small Repair

The helper now waits for one bounded stdin identity ACK before releasing its actual child.
The external test records matching actual birth identity AND userspace liveness, then ACKs.
A 50 ms adversarial consumer delay is not readiness: the real pipe/line and ACK are required.
The original child, signal timeout, owned reap, inherited-parent lock, competitor, release
and post-reap assertions remain. No missing token, zombie or timeout is accepted as a live
identity. Failure is asserted only AFTER actual REAP/RELEASE/helper wait, including controls.
Withheld identity ACK typed-refuses and collects the actual child under the same deadline.
No production cache/resource/authority logic or ceiling changes.

## Deciding Proof And Limits

Actual Linux `cache_owner` passes 3/3, including withheld-ACK and existing readiness-timeout
cleanup. Release-before-ACK ablations each execute one real test and fail live-identity
observation. The deciding negative records child 833032, actual `CHILD_REAPED 833032 0` and
helper exit 0 before failure. The restore passes. Raw hosted Mac failure remains retained.
Linux source/control evidence is not changed-head native macOS success; that execution
and the previously blocked strict Clippy stage remain required.
