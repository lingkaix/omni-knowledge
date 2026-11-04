# License Notice Collection Characterization Tests

This record owns independent characterization coverage for the committed license-notice collector, not product behavior and not permission to change the collector.
The tested subject is the public `scripts/collect-licenses.py` at committed revision `f80813f775d23138496750f6753f61d341aaf65b`, exported as opaque bytes through `git show` and bound by SHA-256 `e3bc710d94b36798f2ac3016dec75c31ad521e0386099ec20b92e60310710376`.
The suite is `tests/tools/notice_collection.py`; it uses only the Python standard library on Python 3.9 or newer.

## Authority And Independence

Every expected value comes from a public owner, never from the subject implementation.
The product owner [project licensing](../specs/product.md#project-license) requires third-party components to keep their original licenses and provenance.
[Dependency guidance](../dependencies.md) requires `Cargo.lock` to fix versions, notices and exact unmodified MPL-2.0 source archives to ship with the bundle, and the generated inventory to state coverage gaps instead of claiming an audit.
Committed revision `f80813f` `scripts/README.md` and its change record `docs/changes/2026-11-04-linux-artifact-notices.md` define the collector contract: default Darwin GUI selection is preserved, `--target <triple> --headless OUTPUT` collects the Linux headless graph, `OUTPUT` must be a new directory, the inventory binds target and selected features and root package and the exact workspace lock digest, retained upstream notice reuse requires matching crate/version/VCS identity and a valid retained hash, and a registry notice gap keeps the exact lock-checksummed original source archive and reports the gap honestly.
The committed CI recipe at that revision runs `python3 -B scripts/collect-licenses.py --target x86_64-unknown-linux-gnu --headless dist/linux-x86_64/ThirdParty` inside the Linux artifact job.
No subject function body, probe output, or future candidate implementation informed an expected value; fixture bytes declared in the test file are the only oracle inputs beyond the owner documents.

## Subject Identity And Instruments

The suite treats the committed collector as opaque bytes.
`export_subject` reads `f80813f775d23138496750f6753f61d341aaf65b:scripts/collect-licenses.py` with `git show` from a checkout that contains the revision, verifies the recorded SHA-256, and fails with operator guidance otherwise; `NOTICE_COLLECTION_SUBJECT_REPO` overrides the search, and the default search includes the sibling lead checkout.
The external Cargo metadata effect is stood in by a synthetic `cargo` shim that records each invocation and serves a static Cargo-format graph; the real collector process, file evidence, hashes, archive bytes, and deciding assertions remain live.
The synthetic workspace declares seven crates: `delta-dev` is dev-only, `epsilon-gui` appears only in the Darwin graph, `gamma-transitive` is transitive through `alpha-core`, and `mpl-lib` declares MPL-2.0.
`retained-gem` carries retained evidence under workspace-relative `vendor/retrieval-kernel/notices/` with a `.cargo_vcs_info.json` revision matching the registry VCS identity.
Fixtures are disposable synthetic workspaces under the ignored `temp/notice-collection-tests/admission/` root; the root is removed after a fully successful run and retained otherwise so failing observations stay inspectable.
An unchanged older committed source, `a4d77f06fda38f197653ef4ec82425e4df6aa8f3` with SHA-256 `b813b4e73d85016a0a477ac7b77c363c2ee40569fdc47fb7d3fb3feb6875eb77`, is used only to confirm the preserved default selection parity; no Linux/headless claim is made against it.

## Coverage Matrix

One primary representative case pins each owner-contract responsibility.

| Owner responsibility | Test |
| --- | --- |
| `--target x86_64-unknown-linux-gnu --headless OUTPUT` collects the Linux headless graph with a normal (default-features-off) build, and the inventory binds target, features, root package, and the exact workspace lock digest; dev-only edges stay excluded | `test_linux_headless_binds_inventory_and_selects_normal_build` |
| Retained upstream notice reuse requires matching crate/version/VCS revision identity and a valid retained notice hash; the registry URL is not identity | `test_retained_notice_reused_with_matching_identity_and_hash` |
| A registry notice gap keeps the exact lock-checksummed original `.crate` archive and reports the gap honestly instead of claiming discovered text | `test_notice_gap_keeps_exact_source_archive_and_reports_honestly` |
| An MPL-2.0 crate ships its exact unmodified source archive alongside its notice | `test_mpl_source_obligation_keeps_original_archive` |
| The default invocation preserves the Darwin GUI selection (`--filter-platform aarch64-apple-darwin`, `--features gui`) | `test_default_darwin_gui_selection_preserved` |
| The preserved default selection matches the older committed collector source, with the inventory-binding difference recorded rather than claimed as an old pass | `test_default_selection_matches_older_committed_source` |

## Negative Controls

Three material refusals are pinned as observed behavior.
A run against a redeclared retained hash that does not match the retained evidence exits nonzero with `retained notice checksum mismatch: retained-gem-0.9.0/LICENSE` and writes no artifact into the destination.
A corrupted `Cargo.lock` checksum exits nonzero with `crate source archive checksum mismatch: gamma-transitive-0.5.0.crate` and writes no artifact into the destination.
An existing destination directory exits nonzero with `File exists` and leaves pre-existing sentinel files byte-identical.

| Refusal | Test |
| --- | --- |
| Wrong retained notice hash | `test_wrong_retained_hash_is_refused` |
| Corrupted lock checksum | `test_lock_checksum_mismatch_is_refused` |
| Existing destination without overwrite | `test_existing_destination_refused_without_overwrite` |

## Run Evidence

Command: `python3 -m py_compile tests/tools/notice_collection.py`, outcome: success.
Command: `python3 -B tests/tools/notice_collection.py`, outcome: `Ran 9 tests in 0.998s` with `OK` and exit status 0 on the first committed version.
After that successful run the suite removed `temp/notice-collection-tests/admission/` itself.

## Boundaries

These are bounded instrument/regression tests.
They do not prove the real Linux dependency graph, macOS bundling, or legal compliance.
Dev-only exclusion and transitive inclusion are proven against the synthetic graph, not against a real lockfile.
The legacy-source comparison proves selection parity only; it does not claim the older source satisfies the newer Linux contract.
