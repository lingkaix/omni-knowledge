# Ordinary Local Read Adapter

Status: builder implementation. Independent tests and key review are pending.
Owner: [storage identity and mount backend](../specs/storage.md).
Base: `445607ccf495b790c0e490c3f64be8413e67ff9b` on `omni-local-backend-445607c`.
This record does not accept M0, complete M1, or change application authority.

## Scope

New crate `crates/knowledge-local` implements the frozen `MountBackend` for one caller-selected ordinary local directory.
The root workspace loan is the `knowledge-local` member and the lock record that points it at the existing `knowledge-core`, `rustix` 1.1.5, and `sha2` 0.10.9 edges.
No other package version, owner, core, cache, wire, test, or gate file changes.
Linux and macOS share the source. macOS execution is not claimed from this build.

## Necessity And Ablation

The frozen core trait has no body and no default that could pretend to enforce a root.
Private cache helpers already walk with `openat` / `O_NOFOLLOW`, but they are cache-specific and editing them is outside this lease, so the adapter keeps a smaller copy of that walk.
`rustix` 1.1.5 with the `fs` feature is already locked and supplies `openat`, `statat`, `statx`, and directory iteration. A new libc binding or a path-string API is not required.
`sha2` 0.10.9 is already locked. A private hex encoder avoids adding the `hex` crate.
Canonicalize-then-open is rejected because it follows links and races the later open.
Reporting `Residency::Resident` or `capabilities.residency` would advertise the caller's ordinary-local premise as a cloud hydration detector. Both stay unknown/false.
`stable_file_id` stays false. Linux generation is not on this `statx` sample and is not invented. Birth time is recorded only when the kernel actually returns it.
A constructor-minted permission is rejected. Trait methods do not take a policy argument, so one `LocalReadPolicy::decide` seam is consulted per call and again before delivery.
Exclusions and child-owned scopes are one caller-supplied list. A detector for `omni-knowledge.json` or inner mounts is not implemented and is not pretended.
No cursor registry is added. Continuation is a null cursor and a new call.
No write, watch, conditional write, NAS, cloud, tracker, GC, job, service, or MCP surface is added.

## Behavior

The bound root is rewalked and compared with the held descriptor before effects and before delivery.
Listings complete only when every in-scope ordinary name was represented. Non-UTF-8, symlink, special, denial, drift, cancel, and budget stops are partial and cannot be read as an empty directory.
Stat does not read bodies. `open_range` hashes the current file with a bounded buffer, retains the requested overlap, and refuses when the digest or the post-read identity does not match.
Budgets count examined names, hashed source bytes, and reserved memory. They are not an RSS meter or a syscall deadline.
Cancel on stat and range read is `Unavailable::Unknown` because the frozen error enum has no cancel variant.

## Remaining Proof

Linux real-file smoke covers the builder example only.
Independent adversarial tests and macOS execution are still required.
Inner mounts, privileged remounts, and hard preemption of one kernel call remain outside this mechanism.
