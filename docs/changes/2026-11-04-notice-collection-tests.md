# License Notice Collection Characterization Tests

This record owns independent characterization coverage for the committed license-notice collector, not product behavior and not permission to change the collector.
The suite is `tests/tools/notice_collection.py`; it uses only the Python standard library on Python 3.9 or newer.
The default current mode gates the actual checked-out source: it selects one exact subject commit from the checkout running the suite, or from an explicit `--subject-repo`/`--current-ref`, exports `scripts/collect-licenses.py` as opaque bytes through `git show`, and records the SHA-256 of the exported file that the tests execute.
The explicit `--legacy` mode pins the historical characterization to committed revision `f80813f775d23138496750f6753f61d341aaf65b` with SHA-256 `e3bc710d94b36798f2ac3016dec75c31ad521e0386099ec20b92e60310710376`, and uses the older `a4d77f06fda38f197653ef4ec82425e4df6aa8f3` with SHA-256 `b813b4e73d85016a0a477ac7b77c363c2ee40569fdc47fb7d3fb3feb6875eb77` only for its parity case.

## Authority And Independence

Every expected value comes from a public owner, never from the subject implementation.
The product owner [project licensing](../specs/product.md#project-license) requires third-party components to keep their original licenses and provenance.
[Dependency guidance](../dependencies.md) requires `Cargo.lock` to fix versions, notices and exact unmodified MPL-2.0 source archives to ship with the bundle, and the generated inventory to state coverage gaps instead of claiming an audit.
Committed revision `f80813f` `scripts/README.md` and its change record `docs/changes/2026-11-04-linux-artifact-notices.md` define the collector contract: default Darwin GUI selection is preserved, `--target <triple> --headless OUTPUT` collects the Linux headless graph, `OUTPUT` must be a new directory, the inventory binds target and selected features and root package and the exact workspace lock digest, retained upstream notice reuse requires matching crate/version/VCS identity and a valid retained hash, and a registry notice gap keeps the exact lock-checksummed original source archive and reports the gap honestly.
The committed CI recipe at that revision runs `python3 -B scripts/collect-licenses.py --target x86_64-unknown-linux-gnu --headless dist/linux-x86_64/ThirdParty` inside the Linux artifact job.
No subject function body, probe output, or future candidate implementation informed an expected value; fixture bytes declared in the test file are the only oracle inputs beyond the owner documents.

## Subject Identity And Instruments

The suite treats the selected collector as opaque bytes.
Current mode resolves `--current-ref` (default `HEAD`) in the selected checkout to one exact commit, exports the collector with `git show`, verifies the checkout actually holds the selection, and fails without fallback when a selected ref or path is absent; `NOTICE_COLLECTION_SUBJECT_REPO` or `--subject-repo` names the checkout.
`export_pinned_subject` serves `--legacy`: it searches the declared checkouts for the pinned revisions, verifies each recorded SHA-256, and fails with operator guidance otherwise; the search tries the named checkout, then this checkout, then the sibling lead checkout.
The external Cargo metadata effect is stood in by a synthetic `cargo` shim that records each invocation and serves a static Cargo-format graph; the real collector process, file evidence, hashes, archive bytes, and deciding assertions remain live.
The synthetic workspace declares seven crates: `delta-dev` is dev-only, `epsilon-gui` appears only in the Darwin graph, `gamma-transitive` is transitive through `alpha-core`, and `mpl-lib` declares MPL-2.0.
`retained-gem` carries retained evidence under workspace-relative `vendor/retrieval-kernel/notices/` with a `.cargo_vcs_info.json` revision matching the registry VCS identity.
Fixtures are disposable synthetic workspaces under the ignored `temp/notice-collection-tests/admission/` root; the root is removed after a fully successful run and retained otherwise so failing observations stay inspectable.
An unchanged older committed source, `a4d77f06fda38f197653ef4ec82425e4df6aa8f3` with SHA-256 `b813b4e73d85016a0a477ac7b77c363c2ee40569fdc47fb7d3fb3feb6875eb77`, is used only inside explicit legacy mode to confirm the preserved default selection parity; no Linux/headless claim is made against it.

## Current-Subject Selection Correction (2026-11-05)

The committed version pinned every non-legacy case to `f80813f`, so a run could pass while the checked-out source under test was a different commit, and a shallow CI clone without the historical objects could not run the suite at all.
This correction changes only subject selection, subject setup, and diagnostic-prose coupling; every independently authored semantic assertion keeps its subject and strength.
The historical first-version run evidence below remains the preserved f808 proof and was re-verified unchanged through explicit legacy mode.

### Necessity And Ablation Of The Prose Decoupling

`GAP_WARNING_NOTICES`, `GAP_WARNING_TEXT`, and the `REUSED_WARNING_NOTICES` alias carried exact English sentences from the implementation.
The owner contract requires explicit coverage-gap classification: [dependency guidance](../dependencies.md) states the inventory reports notice-coverage gaps instead of treating SPDX metadata as a complete redistribution audit, the committed `scripts/README.md` collector section requires coverage gaps to remain explicit, and the collector change record keeps unresolved license evidence a delivery review obligation.
None of these owners specifies warning sentence wording, so exact sentences were implementation coupling rather than owner oracles.
The retained-reuse case now asserts a nonempty warning list of nonempty strings, and the gap case additionally asserts a nonempty `Coverage note:` line inside the affected crate section of the notice document.
The exact warning equality assertions were ablated in favor of these classification assertions.
`notices` identity, `source_archive` path and digest and byte length, `source_archive_reason`, `versioned_source_url`, archived bytes equality, document notice lines, refusal messages, cargo invocation shapes, and all lock/target/feature/root identity assertions remain exactly as first authored.

### Selection Admission

A selected source digest and commit are the actual executed opaque artifact: the digest is computed from the written export file, the tests execute exactly that file, and each run prints the selected repository, ref, commit, and digest.
The legacy-only OLD/NEW parity test is reported as not selected in current mode instead of silently skipped; `--legacy` selects it together with both pinned revisions and fails when a declared prerequisite is absent.
Combining `--legacy` with `--current-ref` is refused.

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

Corrected-instrument runs on 2026-11-05:
Command: `python3 -B tests/tools/notice_collection.py --subject-repo /home/simonx/.cache/openkit-tmp/omni/lead/omni-knowledge`, outcome: `Ran 8 tests in 0.626s` with `OK` and exit status 0; selected lead HEAD `5ad6caeba915783deff9f3b74d3975ac8c332c1e`, exported digest `e3bc710d94b36798f2ac3016dec75c31ad521e0386099ec20b92e60310710376`, parity test reported as not selected.
Command: `python3 -B tests/tools/notice_collection.py --legacy`, outcome: `Ran 9 tests in 0.820s` with `OK` and exit status 0; both pinned subjects resolved and digest-verified through the declared search.
Command: `python3 -B tests/tools/notice_collection.py --subject-repo <depth-1 clone of the lead checkout>`, outcome: `Ran 8 tests in 0.825s` with `OK` and exit status 0 although that clone holds neither `f80813f` nor `a4d77f0`; current mode needs no legacy history.
Command: `python3 -B tests/tools/notice_collection.py --subject-repo <that shallow clone> --current-ref f80813f775d23138496750f6753f61d341aaf65b`, outcome: exit status 1 with `selected ref ... does not resolve ...; there is no fallback to another repository or pinned commit`, even though the sibling lead checkout holds the revision.
Command: `python3 -B tests/tools/notice_collection.py --subject-repo /home/simonx/.cache/openkit-tmp/omni/lead/omni-knowledge --current-ref no-such-ref`, outcome: exit status 1 with the same no-fallback guidance.
Command: `python3 -B tests/tools/notice_collection.py` with no arguments in this sandbox checkout, outcome: exit status 1 with 7 failures; the selected HEAD `d6e6f2b224fd67ed1c422092f081013197393a03` carries collector bytes identical to the older `a4d77f0` source (exported digest `b813b4e73d85016a0a477ac7b77c363c2ee40569fdc47fb7d3fb3feb6875eb77`), and the current-contract assertions correctly refused that older source.
All synthetic fixtures from the successful runs were removed by the suite; the shallow clone and failed-run fixtures were removed during validation and only the pre-existing probe evidence remains.

## Boundaries

These are bounded instrument/regression tests.
They do not prove the real Linux dependency graph, macOS bundling, or legal compliance.
Dev-only exclusion and transitive inclusion are proven against the synthetic graph, not against a real lockfile.
The legacy-source comparison proves selection parity only; it does not claim the older source satisfies the newer Linux contract.
Current mode gates whatever checkout it selects: a checkout whose collector predates the owner contract fails with semantic failures instead of being silently excused, as the no-argument sandbox run above shows.
