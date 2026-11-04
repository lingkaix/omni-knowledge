# Independent Test Disposition And Acceptance Plan

This record owns M0 test evidence and replacement obligations, not product behavior or permission to retire tests.
The assigned outcome is an independent preservation baseline and a specification-first acceptance plan for the full-version delivery.
Production implementation is outside this author's scope.

## Authority And Independence

Expected observations come from the committed [product](../specs/product.md), [storage](../specs/storage.md), and [search](../specs/search.md) owners.
The [test strategy](../specs/test-strategy.md) and [test-author role](../roles/test-author.md) govern independent replacement and retirement.
The accepted full-version design and supervisor amendments distinguish preserved behavior from changed intent.
The service acceptance plan will cite committed DTO owners before it becomes executable.

The OLD code identity is `fa7a3c9d974c8a8e622e6e9ad88105c6aa6f781b`.
Initial HEAD was identical to OLD.
The doctrine-only successor is `25f9d69c5e53763922a072a2ef94bdd0b8af381f`.
Only existing public integration tests and declaration-only extraction from OLD `src/lib.rs` informed API use.
No production function body or future candidate implementation informed an oracle.

## Preservation Matrix

All existing tests remain in place.
A row identifies coverage to preserve, not permission to delete every test in its file.
Preservation clause names refer to OLD owner headings; the [full-version criterion map](2026-11-04-full-version.md) identifies their current successors and intentional changes.
One public-interface regression is primary for each invariant; broader observations require a distinct seam risk.

| Required behavior and owner clause | Existing coverage | M0 addition or disposition | Replacement obligation |
| --- | --- | --- | --- |
| Storage / Mutations: import copies, preserves sources, and never overwrites | `knowledge_base::import_preserves_sources_and_rejects_collisions`; `file_actions::transfers_refuse_file_and_metadata_collisions` | Keep; add `preserved_behavior::competing_imports_preserve_sources_and_the_winning_destination` for simultaneous distinct payloads, winner metadata, reopen, and retry | Replacement must preserve both original byte strings and the complete winning destination on OLD and NEW; a happy-path copy is insufficient |
| Storage / Authority and Search / Strategies: authored spelling, frontmatter/annotation union, complete all-required tags, case option, filter before limit | `knowledge_base::binary_metadata_and_exact_tags_are_searchable`; `unified_search::existing_tag_filters_fold_case_without_reindexing`; `unified_search::peer_tags_and_text_are_case_insensitive_alternatives` | Keep; add `preserved_behavior::authored_tag_union_keeps_spelling_and_requires_every_complete_filter` for case-distinct authored tags and two required filters with earlier distractors | OLD/NEW proof must retain raw authored text, both tag spellings, exactness, AND behavior, and limiting after eligibility |
| Search / Strategies: canonical literal/regex completeness and deterministic source coordinates | `knowledge_base::grep_short_patterns_and_regex_alternatives`; `knowledge_base::literal_trigram_candidates_are_verified_without_false_negatives`; `unified_search::unified_text_is_literal_while_cli_grep_keeps_regex` | Keep; add `preserved_behavior::regex_enumerates_all_fixture_lines_and_literal_punctuation_stays_literal` with complete expected coordinate sets rather than nonempty-hit assertions | Preserve short CJK, substring-inside-token, alternatives, case behavior, literal punctuation, and line-local matching on OLD and NEW |
| Search / Unified Desktop Query: dense file votes, RRF constant 60, retained evidence | `unified_search::combined_results_preserve_unique_locations_and_real_coordinates`; `unified_search::single_method_preserves_existing_retrieval_ranking`; existing internal fusion checks | Keep; add `preserved_behavior::fusion_votes_once_per_file_while_retaining_every_distinct_line` through real indexing and query owners | Required public score formula and file order must survive changing repeated line count; do not preserve private fusion helper calls |
| Product / Source Authority; Storage / Authority: confinement, child roots, unknown files | `knowledge_base` and `file_actions` ownership, symlink, marker, and binary cases | Keep without duplicate suite | New mount identities may change setup, not confinement or original-byte preservation |
| Storage / Index Publication: whole-generation publication, reader lifetime, failed work, retained unknown files | `knowledge_base` generation/provider failure cases; `search_cache` publication and containment cases | Keep; no speculative rewrite | Coupled checks of still-required public behavior need OLD/NEW replacement proof; purely retired cache layout needs independent disposition; external cache location is changed intent |
| Search / Scope And Evidence: source/derived line and recording-time coordinates | `extraction` line/preview cases; `knowledge_base` transcript cases; `embedding` timed media cases | Keep without duplicate suite | Real processing producer and retrieval consumer must agree; a fabricated retrieval unit alone cannot replace this crossing |
| Processing identity, bounded extraction, safe preview/export | `embedding` identity/vector/failure checks; `extraction` format, escaping, and no-clobber checks | Keep without duplicate suite | Real local parsers and installed media binaries remain subjects; deterministic providers prove protocol, not model quality |
| Public CLI/MCP, live metadata, status, and runtime behavior | `mcp`, `file_info`, `status`, and `runtime` integration suites | Keep; distinguish intended exposure changes from regressions | A new transport assertion does not replace storage or retrieval assertions; macOS-only effects need their real platform |
| Native desktop interaction | Existing GUI-gated tests and native observations | Keep; not selected on Linux | macOS CI must build GUI, run headless and FSEvents checks, and retain an unsigned app artifact; IME, real providers, and user prompts remain native observations |

## Evidence Rules

For a coupled check of still-required behavior, run its replacement against exact OLD and final integrated NEW with the same scenario and deciding assertions.
Record both identities, commands, outcomes, and any mechanical setup adaptation.
A doctrine-only successor or unchanged production tree does not prove parity with a later refactor.
A new requirement that intentionally fails OLD is not a preservation replacement.
Pure internal-mechanism retirement needs independent disposition and coverage accounting, not an invented public requirement or duplicate test.
No test is retired by this record.

The independent suite uses the real public `KnowledgeBase` operations and disposable authored files.
It does not write private index tables, fabricate durable receipts, inspect hidden helpers, or depend on real libraries or credentials.
The concurrency check permits either importer to win; it does not prescribe thread scheduling.
The fusion oracle derives scores from the owner formula, not production output.
Exact coordinate sets derive from the synthetic input lines and the line-matching contract.

## Baseline Status

Environment: Linux x86_64, Rust 1.99.0, Cargo 1.99.0, FFmpeg and FFprobe n9.0.1.
Builds use `CARGO_BUILD_JOBS=2` and an author-private `CARGO_TARGET_DIR`.
Native GUI and macOS-only compilation cases are not selected on this Linux host; they are not counted as passing or ignored tests.
No real-model, cloud, remote Git, or personal-library scenario is selected.

| OLD observation | Exact command or instrument | Result |
| --- | --- | --- |
| Collected inventory | `cargo test --workspace --locked -- --list` | 133 headless tests; 12 binaries including the zero-test CLI binary; zero doctests |
| Format | `cargo fmt --all -- --check` | Exit 0 |
| Default suite | `cargo test --workspace --locked` | Exit 101; library target 22 passed, 3 failed; Cargo stopped before remaining targets |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Exit 0 |
| Headless release | `cargo build --workspace --release --locked` | Exit 0 on exact OLD; artifact SHA-256 `9ee854ede57aadaf9e0704a32e93805d073b4cfd658c732040eafcfdf238402a` matches the previously smoked binary |
| Release artifact smoke | Built `omni --help`, `init`, `add`, `index`, and `search --mode grep --json` | Real binary returned the synthetic source at line 1; original preserved; disposable library removed |
| Complete serial diagnostic | Every binary from the OLD collected inventory, each with `--test-threads=1` | 132 passed, 1 failed, zero ignored or filtered; not a replacement for the default-run RED |
| Independent preservation on exact OLD | `cargo test --locked --test preserved_behavior`, before adding the defect module | 4 passed, zero failed or ignored |
| Complete default diagnostic on exact OLD plus four new checks | `cargo test --workspace --locked --no-fail-fast` | 136 passed, 1 failed, zero ignored or filtered; original intermittent lock failures remain retained |

The OLD inventory comprises 25 library tests, 13 embedding, 11 extraction, 11 file actions, 5 file information, 29 knowledge-base, 2 MCP, 3 runtime, 10 cache, 16 status, and 8 unified-search tests.
This is actual libtest collection, not a textual count of test attributes.
The complete diagnostic runs account for every selected original case even though the first default command stopped early.
A daemon interruption stopped one additional exact-OLD release invocation without a terminal result; that interrupted log is retained, not counted as a pass.
The recovered exact-OLD build completed, and its binary hash equals the already tested release artifact.
The four-test preservation source hash on OLD is `6ea2f968fbe1f7e88d903fa155a59ee0adbdbed33b727c98ef766eb1dbd5786e`.
Those four test bodies remain unchanged after adding the separately classified writer-release module and clarifying the module documentation under doctrine r2.

### Failure Dispositions

**Runtime portability test defect:** `runtime::tests::fixed_tool_directories_work_with_limited_path` assumed that `/usr/bin:/bin` contains no FFprobe.
The declared media-test prerequisites explicitly permit installed FFprobe, and the product requires fixed-directory and PATH discovery.
The independently authorized correction supplies an empty temporary PATH directory for that absence case.
It retains all five assertions: fixed-directory precedence, PATH fallback, missing optional tool, missing requested tool, and rejection of relative PATH entries.
The isolated fallback still contains a synthetic executable FFprobe.
No runtime production code or expected value changes.
The exact corrected test passes, with the original failure retained.

**Executable-fixture setup defect:** a later final-head parallel batch had 137 passes and one existing runtime failure, despite an earlier 138-pass batch.
`nonzero_launcher_exit_reports_uncertainty_and_preserves_helper` did not receive the expected exit status 9.
An assertion diagnostic, without an expected-value change, exposed `Text file busy` before the synthetic launcher started.
Ten Cargo diagnostics passed; a separately bounded direct-binary diagnostic reproduced the same failure on trial 10 and stopped.
The original failure and every diagnostic outcome remain retained; those passes do not erase the red batch.

The fixture wrote executable scripts in the multithreaded parent, where an unrelated fork could inherit a writable descriptor.
A controlled OS probe reproduced error 26 with that inherited descriptor and exit 9 after its child exited.
Writing the same fixture bytes in a separate child allowed exit 9 while the unrelated child remained held.
This establishes the setup hazard without claiming a syscall trace of the original intermittent occurrence.
`Fixture::new` now writes both scripts in an isolated shell child and waits for its exit before setting the same modes.
Paths, script bytes, deadlines, scenario, and every assertion remain unchanged; the assertion now also prints an unexpected actual error.
No parent writable executable descriptor can reach an unrelated fork, and no serialization, sleep, retry, or production change is added.
Without this isolation, the observed executable setup can fail before the intended launcher scenario begins.

**Writer-lifetime product defect:** two original parallel file-action tests reported another active writer; both passed in the complete serial diagnostic.
A later complete default run did not reproduce those two intermittent failures.
This does not justify serializing the suite or changing their expectations.
A separate public-operation probe deterministically established a writer-lifetime violation in three trials.
It holds an external provider response while real indexing is active, confirms competing metadata rejection, and suspends an unrelated child before exec.
After the provider fails and indexing returns, metadata still incorrectly fails until that unrelated child executes.
The probe releases and reaps the child before deciding assertions.
These observations establish a product defect consistent with the intermittent symptoms; they do not claim a syscall trace of each original failure.

`tests/contracts/writer_release.rs` retains that public regression under the existing Cargo harness.
It fails OLD and the pre-fix current code for the owner-required release behavior.
It is an existing-defect regression, not a passing preservation replacement and not an intentionally new product requirement.
The production correction belongs to the lead; this author changes no lock implementation.
Without the held provider response and child gate, the relevant active-operation and inherited-child intervals become timing-dependent.
Those two controls are fixture necessities, not new production mechanisms or a new test runner.

### NEW Evidence And Landing Scope

Doctrine r2 at `87fd6ab60409cd3fa681cbfbf6e7c59005e0fa40` was pulled and all assigned doctrine owners were reread.
The lead-owned writer correction landed in `56fa2e3`; this author did not inspect or modify its production implementation.
On reviewed base `f581c9b`, the unchanged public assertions pass: `cargo test --locked --test preserved_behavior` reports 5 passed and zero failed or ignored.
The exact runtime portability check also passes; repository-wide format and Clippy pass on that composed tree.
The final committed-head inventory, complete headless suite, release build, targeted checks, and raw artifact paths are recorded in the external test-author evidence report.
No OLD failure is erased by these NEW observations.

This landing changes four preservation tests, one independently reproduced defect regression, and only authorized runtime test-module functions.
It changes no production code, dependency, or public schema, and retires no existing test.
The test-to-production line ratio is intentionally unbounded because this is an independent tests-only assignment, not a production feature patch.
Future retrieval or storage refactors still need the replacement proof described above; a pass against the writer correction does not pre-approve them.

## Design-New Acceptance Plan

The accepted [service wire contract](../specs/service.md) and full-version product owner landed at `28df447f4d7ddf1415155d9aa71f9f63e0eb3815`.
Their complete DTO declarations and behavior clauses were read before this plan was written.
The complete [storage](../specs/storage.md), [history](../specs/history.md), [processing](../specs/processing.md), [search](../specs/search.md), and [package](../specs/package.md) owners landed at `795e4a54ca1cdf9caf9be17456ee627d55f5b3fe` and were read before landing this plan.
All service/product amendments since the previously read revision were also reviewed, including root browsing, clone/schedule DTOs, attribution fields, and EPUB operations.
The later `8f8456e42a6af89b7e0494b0e5d37f328c263890` read-only verification amendment supplies the browse-to-read acceptance sequence below.
The following new requirements are **intended-red on OLD and not yet executed**, not preservation replacements or claims of implementation completion.
Specific tests must use the committed domain owners and public crate seams when those seams become available.
No future production implementation may supply the expected result.

L1 means a public module with its real local store; L2 means the real producer and consumer together; L3 means the actual built process and transport.
Only external effects outside the subject may use controlled providers, clocks, or fault adapters.
Owner operations create valid state before deliberate corruption or interruption.
One primary observation belongs at the lowest sufficient layer; a higher-layer observation is included only for a distinct crossing risk.

| Milestone and owner | Lowest sufficient observation and public seam | Deciding assertion and negative case |
| --- | --- | --- |
| M1: Product / Full-Version Owners; service `Library`, `MigrationPlan`, `Source` | L1 core open/create/migration with nonempty legacy and unmarked folders; L2 migration writer to reopened library reader | Opening alone creates only an in-memory library handle, not a manifest; persistence retains that ID; migration preview names full path map and orphan annotations; conflict leaves old authored bytes intact; verified apply retains spelling, extension fields, and source bytes |
| M1: Product / Mounts and identity; service `Mount`, `Source`, `Coverage` | L1 local mounted directories plus controlled unavailable/residency adapter; public browse/refresh/details | An incomplete, denied, or offline enumeration never creates deletion; rename continuity differs from equal-hash duplicates; placeholders and unknown cloud residency do not cause background reads; detach preserves originals |
| M1: Product / Authored writes; `ExpectedVersion`, `WriteResult`, `edit_text`, `transfer` | L1 real filesystem with controlled competitor and interruption between safe public stages | Null expected version means no-clobber creation; stale digest or identity retains both versions and reports conflict; reopen reports recovery instead of blind replay; transfer preserves annotations and rejects destination collisions |
| M1: Product / Cache and tracking; service `Status`, `Job`, `refresh`, `process`, `cancel` | L1 real bounded cache/jobs; L2 source observation to query/read validation; native adapter tests for inotify and FSEvents | Cache is outside every home/mount/sync root; durable history is separate and outside cloud sync; eviction respects pins and changes coverage; duplicate jobs do not duplicate effects; overflow/startup reconciles; pending work, offline wait, cancellation, and supersession remain distinct |
| M2: Product / Text history; `WriteResult`, `HistoryEntry`, `history_list`, `history_diff` | L2 real filesystem operation to system Git, disposable signing keys, isolated user configuration | One durable text operation creates one commit; unchanged operation creates none; actual signature verifies; missing/locked signer preserves candidate and returns `signing_required`; explicit unsigned creation stays visible; reads and generated work create no commits |
| M2: History / Git-Server Library And Schedule; `library_clone`, `Proposal`, `confirm_merge`, `SyncResult`, `SyncSchedule` | L2 real private Git history and local bare remote; one L3 human CLI confirmation check | Agent operation proposes without advancing accepted home; confirmation binds complete diff/base/proposal; stale tuple refuses; fetch/divergence/push refusal retains local commits and remote authorship; no hooks, force reset, or unsolicited remote contact; existing staged work survives; remote rights and disabled/no-push schedule defaults are observable |
| M2: Service / Jobs, Mutations, History; `operation_status` | L2 interrupt real file installation/ref advancement, reopen, and inspect files plus refs | Retrying the same operation does not create a second commit; ambiguity stays `indeterminate`; local completion never claims push or cloud synchronization |
| M3: Product / Processing; service `Locator`, `Evidence`, `read_representation` | L2 actual parser/media producer to retrieval consumer using tiny licensed or generated documents, transcripts, and installed ffmpeg/ffprobe | Direct text coordinates remain original; converted coordinates bind representation digest; timed evidence uses original recording clock; correction invalidates dependent output; malformed/oversized content preserves metadata search and explicit coverage |
| M3: Product / Processing; service method-specific errors and model status | L1 declared provider boundary with deterministic local HTTP responses; one real installed-tool process check | Wrong identity/dimension, nonfinite/zero vectors, malformed responses, and timeouts fail without publishing mixed output; lexical operation remains independent; deterministic vectors do not claim semantic recall or WeMM qualification |
| M4: Product / Retrieval; service `SearchRequest`, `SearchResult`, `FileHit` | L2 real mounted/processed inputs through the pinned kernel and public retrieval facade | English/Chinese/Japanese golden matches, canonical byte/case and LF-line regex behavior survive; all admitted partitions are scanned; exact/regex results are not restricted to BM25 candidates; rank fusion deduplicates files and retains locators |
| M4: Service / Search DTOs | L1 public query with deliberately early ineligible files and repeated evidence | `required_tags` filters before distinct-file limit; keyword tags remain alternatives; per-method errors name the selected method; scores are finite; top-k and enumeration coverage differ; unverified discovery is `metadata_observation`, never a verified body match |
| M4: Product / Retrieval and tracking; service `Coverage` | L2 actual immutable lexical/vector publication and current source verification, with interrupted build and pinned reader | Failed generation work retains the previous complete tuple; changed/unavailable originals are withheld before delivery; model/processor identity cannot mix; bounded exact-vector reads avoid loading a full JSON corpus; optional Lance adoption needs separately registered measurements |
| M5: Product / Frozen packages; `PackageSelection`, `PackagePlan`, `PackageResult` | L2 real snapshot writer to archive reader to retrieval kernel; deny extraction in the compatible STORED-index scenario | All three presets use one native format; snapshot identity binds canonical selected content; media/index methods are STORED; selected index remains searchable without extraction; incompatible optional indexes have an explicit fallback reason |
| M5: Product / Frozen packages; service plan/apply contracts | L1 hostile archive validation plus real restore into a disposable new home | Traversal, duplicate/case-colliding names, special entries, quota violations, length/header disagreement, and digest failure refuse before installation; sparse fixtures exercise offsets above 4 GiB; interrupted capture never becomes a completed archive; stale destination plans do not overwrite |
| M5: Product / Interoperability; `v1_import_plan`, `v1_export_plan` | L2 genuine committed foreign fixtures through mapping and native readers | Core bytes/links/digests survive valid mapping; missing genuine host context returns `compatibility_refused`; foreign claims grant no local access and cannot survive changed content falsely; references do not pretend to restore unavailable originals |
| M5: Service / Package DTOs | L1 reference-only package selection and integrity inspection | Unverified references remain uncovered; `require_complete` refuses missing hashes; signature trust differs from entry verification; imported mount declarations never cause backend contact; plans bind captured selection and versions |
| M5: Package / EPUB Publication Boundary; `EpubSelection`, `epub_plan`, `epub_export` | L2 real service export to EPUB reader with ordered Markdown/HTML and explicit contained media | Output navigation/chapters reopen correctly; empty media selection copies nothing; wrong plan kinds, stale destinations, escaping resources, and overwrite races refuse; no native snapshot/signature claim |
| M6: Service / Encoding And Compatibility | L1 public DTO parser/encoder and dispatcher with committed valid/invalid JSON fixtures | Root browsing alone permits the empty path sentinel; mutations still require nonempty confined paths; exact U64 bounds and canonical decimals survive round trip; unknown fields/variants/versions and escaping paths refuse before effects; required nullable fields remain required; range and frame limits are enforced; no test depends on diagnostic wording |
| M6: Service / read-only verification | L2 `browse` → `verify_source` → `read_range` using a real unindexed resident source and returned `ExpectedVersion`; one L3 read-only MCP crossing | Exact fixture digest and bytes are returned without manifest/pointer creation, text commit, or write permission; details alone does not hash; drift, excluded paths, placeholders, unauthorized unknown residency, and exhausted budget refuse without a fabricated digest or hydration |
| M6: Service / Transports And Exposure | L3 exact headless artifact through owner IPC, CLI JSON, direct stdio, daemon stdio proxy, and enabled loopback HTTP | Boot/reuse leaves one writer; actual request IDs/result variants agree; stdout is protocol-only; CLI exits match typed outcomes; MCP catalog excludes owner operations; Host/Origin/framing failures return their specified status; no GUI/display dependency |
| M6: Service / Transports And Exposure | L3 selected libraries with excluded synthetic secret markers; revoke exposure during a held response | Read-only rejects writes; root/author/confirmation injection cannot widen authority; excluded names/counts/snippets/history/bytes stay absent; sensitive libraries never appear over HTTP; changes fence controlled pending delivery; no logs/journal record source bodies or queries |
| M6: Service / operation IDs and jobs | L3 disconnect/restart after accepted work; inspect `operation_status`, `job_status`, and owned effects | Same normalized operation ID returns the same outcome/job; changed arguments give `operation_mismatch`; cancellation does not invent rollback; restart does not publish false completion or duplicate a durable effect |
| M7: Product / Desktop Workspace; service shared DTOs | L2 GUI service-client contract, authored by the assigned GUI owner; native app observations on macOS | Saved values and pending reconciliation differ; failed queries retain last successful applied settings; no private GUI mutation/retrieval path; line/time selection uses returned evidence rather than fabricated offsets |
| M7: Product / Delivery And Qualification | macOS arm64 CI build/headless/FSEvents tests plus unsigned app artifact; separate native human checklist | Build and artifact are actually observed; IME, Finder/Quick Look, real cloud/NAS, WeMM GPU/ANE, Keychain, and Gatekeeper remain not qualified until their real environment is used |
| M8: Product / Acceptance Examples | Accounted complete deterministic batch, final artifact smoke, and limited real compositions of the three generic library setups | Restore then reopen/query, signed local change then remote divergence/proposal, and interrupted mounted-media work then recovery preserve the domain invariants together; these stories do not replace lower-layer negative cases |

The new `limit` counts distinct files and excludes zero, unlike the legacy evidence-record bound; tests for this are intentional behavior-change checks.
External cache placement, unmarked opening, live stale-result withholding, visible annotations, signed history, and read-only MCP defaults are also changed intent.
Do not rewrite old assertions to call those changes preservation.
The byte-exact kernel literal interface and escaped case-insensitive desktop literal interface need separate cases rather than a silent case-semantics substitution.

For M8, retain the actual selected inventory, disjoint batch membership, code identities, ignored/opt-in cases, and all terminal outcomes.
A failed or interrupted target is not removed from the denominator.
Mac CI and real-device observations remain separate from the Linux results in this record.
