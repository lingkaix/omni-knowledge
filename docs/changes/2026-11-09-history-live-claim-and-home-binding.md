# Bind native History effects to actual work and selected home

The integrated local-remote/quiet-batch/schedule slice uses `HistoryWork { claim:
&Arc<Claim>, key }`. Native effects validate the original generation with Core's
`Claim::stop_requested`, independently of matching job numbers, deadlines and
reservations. The original same-Claim Holding is split before descriptor copies.
The existing Call returns collected credit to that generation; its existing native
Pending retains the actual Holding and Arc<Claim> when group, pipe, reap or lock
collection remains unknown. No extra work engine or liveness token is introduced.

External batch and schedule APIs accept a concrete `LocalMount<P: LocalReadPolicy>`.
They compare its current held native root identity with the selected History home
before reads, deletion observations and publication. An unrelated empty local root
cannot authorize deleting a selected-home history entry.

Integration preserves the runtime's native installed-Git selection for both local
plumbing and remote commands, including the existing Darwin launcher avoidance.
Local transport fixtures do not qualify SSH/HTTPS, production host routing, home
file installation, native macOS prompts, or installed artifact acceptance.
