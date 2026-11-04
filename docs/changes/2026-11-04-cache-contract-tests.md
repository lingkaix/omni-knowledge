# Knowledge-Core Cache Contract Tests

This record owns the independent integration tests for the public cache seam.
It does not accept the cache, amend storage, or add garbage collection.
The tests are `crates/knowledge-core/tests/cache_contract.rs`, `crates/knowledge-core/tests/cache_owner.rs`, and the support modules under `crates/knowledge-core/tests/cache/`.
Production lines in this change: 0. The lease forbids production edits, so the test-to-production ratio is not a reason to add implementation.

## Authority And Independence

Expected observations come from the accepted [storage](../specs/storage.md) cache, identity, and lifecycle clauses, the cache section of the [crate guide](../../crates/knowledge-core/README.md), and the rendered public cache API.
The candidate bodies in `src/cache.rs` and `src/cache/native.rs`, builder smoke scripts, smoke results, and oracle tables were not inputs.
A failing check stays a failing check. This record does not retune an expectation to obtain a pass.
Early executions that failed because the premise admitted the wrong source, reused an installed key for a disk ceiling, or compared a chmod-stale directory identity were corrected in the fixture. The product assertions stayed in place.
The mapping premise hashes real synthetic fixtures and supplies known roots and locality. It is not provider discovery or service authorization.

## What The Suite Decides

Cache identity is the verified original digest plus the effective processor fingerprint. Map insertion order is not identity. Changing options, dependencies, transcript bytes, annotation bytes, version, or code changes the fingerprint. Empty required identity is `Invalid`. An oversized fingerprint is `Budget`.
Equal derived bytes are not an asset. The same payload under a different admitted original digest is a new install. An unadmitted source id receives no bytes, and holding the digest is not a lookup.
A private 0700 root outside home, mount, cloud, and history admits and publishes. Overlap with each of those classes, a prohibited directory inside the cache, and a lexical `sibling/../cache` alias refuse with `UnsafeRoot` before effects.
A symlink root, a symlink path component, modes other than 0700, and `Untrusted` locality refuse before effects. `ExplicitLocalAlternative` admits only after the test re-observes that same 0700 directory.
Unresolved and symlink-spelled prohibited roots fail closed before effects. That is the current slice behavior, not a storage amendment that offline roots are outside the cache.
Publication is atomic and no-clobber. A duplicate payload is `AlreadyPresent`. A different payload at the same key is `Conflict`. Bounded reads check a short head, a middle range, a short tail, and an empty read at EOF. A read past `max_read_bytes` is `Budget`.
Corrupting payload-sized files makes a later read fail. Republishing the original bytes does not repair those files and does not make the object readable. The test does not require the directory snapshot to be unchanged, because a failed publication may retain a private candidate.
Cancellation, a failing producer, and an object-byte ceiling keep the prior complete object. A separate disk ceiling, on a new processor key whose payload exceeds `max_disk_bytes` while staying under `max_object_bytes`, returns `Budget` and leaves the prior object readable.
Revision, content, domain, policy refusal, home identity, recipe, and a prohibited root set that gains the cache directory withhold bytes. Restoring the previous admission makes the same pin readable again, except the prohibited-root case, which ends the test.
Replacing the live root does not move effects to the new directory. Domain, library, and copied-home namespaces install and read their own bytes.
Unknown files and symlinks stay, are not followed, and survive reopen. A cancelled publish of a new processor key retains a charged candidate across reopen. An unknown directory makes accounting return `InspectionRequired` and is not deleted. Open may return that error or admit and then report it from `usage`. After the caller removes only that directory, the prior object reopens.
Entry, zero-buffer, pin, identity, disk, and in-process re-entrant admission are finite. A second open during an active publish is `Busy`.
The cross-process helper is a single-threaded public consumer. Its pass, fail, and timeout stand-ins admit the observer before either product case. An unrelated child blocked before exec inherits owner descriptors. The parent owner and pin exclude a competitor. After the parent drops the cache and then the pin, the competitor opens while that child is still before exec. A child that runs destructors after fork does not unlock the live parent. The competitor opens only after the parent releases. Readiness is a pipe or a stdout line. Cleanup signals only a pid whose `/proc` start token still matches. An unreaped `_exit` is `Z`, not a live owner.

## Coverage Matrix

| Owner or public clause | Test |
| --- | --- |
| Test hasher matches published SHA-256 vectors | `synthetic_digest_matches_published_sha256_vectors` |
| Structural processor fingerprint, order, and ceilings | `processor_fingerprint_frames_order_options_and_dependencies` |
| Same bytes are not asset identity; no naked digest grant | `same_bytes_do_not_admit_another_source_or_a_naked_digest` |
| Private local root outside every root class | `private_local_root_admits_outside_every_root_class` |
| Overlap with home, mount, cloud, and history | `overlap_with_each_root_class_refuses_before_effects` |
| Symlink, mode, untrusted locality, explicit local alternative | `symlink_permission_and_untrusted_locality_refuse_before_effects` |
| Unresolved or symlink-spelled prohibited roots fail closed | `unresolved_or_symlink_prohibited_roots_fail_closed_without_effects` |
| Atomic no-clobber install and bounded readback | `publish_is_atomic_no_clobber_and_range_verified` |
| Corrupt objects are not repaired | `corrupt_objects_are_not_overwritten_or_repaired` |
| Cancelled, failing, and over-budget producers keep the prior object | `cancelled_failing_and_over_budget_producers_keep_the_prior_object` |
| Mapping, recipe, and root-set changes withhold bytes | `mapping_recipe_and_root_changes_refuse_bytes_without_dropping_the_object` |
| Captured root replacement does not redirect effects | `captured_root_replacement_does_not_redirect_effects` |
| Domain, library, and copied-home isolation | `namespaces_isolate_domain_library_and_copied_home` |
| Unknown files, symlinks, and failed candidates are retained | `unknown_entries_symlinks_and_failed_candidates_are_retained` |
| Finite entry, disk, pin, and I/O admission | `finite_entry_disk_pin_and_io_limits_refuse` |
| Observer pass, fail, and timeout | `observer_admits_pass_fail_and_timeout` |
| Inherited pre-exec descriptor does not extend owner lifetime | `inherited_preexec_descriptor_does_not_extend_owner_lifetime` |
| Post-fork child destruction does not unlock the parent | `post_fork_child_destruction_does_not_unlock_the_parent` |

## Gaps That Are Not Cache Amendments

No test deletes a cache object. Garbage collection, eviction, recovery deletion, a job queue, a root detector, service authentication, and full storage integration are absent from this slice and are not given a passing case.
Storage still expects an offline prohibited root to be excluded rather than to block admission forever. This slice fails closed on an unresolved or symlink-spelled prohibited root. The passing test records that slice behavior. It does not narrow the storage rule.
The tests do not prove durability across power loss, a real NAS or cloud folder, extended ACLs, or an unprivileged host. The process cannot chown a directory to another user, so a non-owner UID refusal was not executed. Exact 0700 and other-mode refusals were executed.
`Indeterminate` is not forced. No test claims atomic protection against a same-UID process that renames the root during a syscall, or against a privileged remount.
An earlier execution saw an additional regular file after a refused republish of a corrupted object. The corrupted payload-sized files kept their flipped bytes. The committed test requires those files to stay unchanged and unreadable. It does not freeze private names or framing bytes.
The unknown-directory case passes whether `open` returns `InspectionRequired` or `usage` does. This package run did not record which arm the product took. Neither arm deletes the directory.
The forking helper is 301 lines. It is not a controller framework. Its stand-in modes are not product evidence.
Independent key review has not accepted this suite.

## Checks

The package build used Rust 1.99.0, Cargo 1.99.0, `--locked --offline`, two Cargo jobs, and a private target directory and `TMPDIR`.
The knowledge-core package does not compile the vendored kernel, so no dictionary cache was required.
`cargo fmt --all -- --check` passed.
`cargo clippy -p knowledge-core --all-targets --locked --offline -- -D warnings` passed.
`cargo test -p knowledge-core --all-targets --locked --offline` passed the 18 new cache tests and, in the same invocation, the existing 26 core tests. Nothing was ignored.
The library target and the manifest example target each reported 0 tests. Those are empty harnesses, not a filtered acceptance run.
The example binary was not executed and is not an oracle.
Independent key review has not accepted this suite.
