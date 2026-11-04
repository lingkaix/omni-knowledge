# Engineering Doctrine Synchronization Record

Date: 2026-11-04. Initial implementation complete. The supervisor subsequently supplied an independent changes-requested review and accepted F1–F6/N1; the r2 correction mapping below records the follow-up. This record preserves execution evidence and cannot authorize product behavior or independently accept its writer's work.

## Source Intent And Boundary

The engineer requested the same engineering doctrine in Omni Knowledge and OpenKit, especially testing changes since 2026-10-14. The supplied brief authorizes documentation edits, coherent commits, and a push to Omni Knowledge main. It forbids source-code changes, starting any agents, and sending messages to coordinators. Those restrictions prevent a separate review context during this execution; self-review is not reported as independent acceptance. The sole context performed the writer function under the existing role contract, without dispatching an agent.

OpenKit source: `b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1` through its read-only `origin/main`. Omni baseline: `fa7a3c9d974c8a8e622e6e9ad88105c6aa6f781b`. The [adoption decision](../decisions/2026-11-04-engineering-doctrine-sync.md) records reasons and deliberate exclusions. Current authority lives in the root contract and linked engineering owners, never in this record or the external report.

## Source Review

Read all existing Omni engineering owners and the product-oriented agent operations guide. Read OpenKit root execution, contribution, engineering doctrine, execution and rationale, verification instruments, documentation model, writing, toolchain, Core doctrine, and all role contracts. Also read the test strategy and October 22 and October 31 decisions. Inspected history from 2026-10-10 for those owners and the testing and calibration specifications, plus relevant commit diffs.

The principal testing change is `bf809d764` on 2026-10-22. The rebuild-corpus and falsifiable-delegation changes are in `c90b2d328` on 2026-10-31. Temporary-directory changes `a1f2c6b8e` and `41830d22d` contribute general cleanup and isolation requirements, not JavaScript infrastructure. Core renames and product audit amendments remain product-specific except for general ownership and authority protections.

## Retained-Criterion Mapping

Locations below refer to headings at the baseline unless marked new. Unlisted text stays unchanged in meaning; reflow joins prose within a paragraph without dropping criteria. The role split removes the old `docs/agent-roles.md` owner rather than retaining a parallel copy.

| Old location or source | Disposition | Current destination or accepted reason |
| --- | --- | --- |
| AGENTS, Work And Evidence: lowest sufficient regression | Clarified | Same section routes public-interface test selection to test strategy. |
| AGENTS, Work And Evidence: evidence limits and justified exceptions | Split responsibility | Verification retains evidence; test strategy owns tests and lifecycle. |
| AGENTS, Work And Evidence: never weaken a contract-derived failure | Retained | Same section; independent disposition is not a builder bypass. |
| AGENTS, Work And Evidence: actual diff, quality, independent judgment | Retained and extended | Same section adds the final-head gate link. |
| AGENTS, Communication: English, Markdown, STE style, no prose wrapping | Retained and extended | Same section; brief also prohibits Vietnamese. |
| CONTRIBUTING, Verify And Review: final diff, consumers, exact checks, limits | Retained and extended | Same heading links local commands, lifecycle, and final-head push gate. |
| agent-roles opening: questions and limits of delegated functions | Moved | roles/README, opening and Functions. |
| agent-roles: primary position versus role function | Moved unchanged | roles/README, opening. |
| agent-roles, Common Contract: root, owners, local guides, direct artifacts, assigned effects | Moved and clarified | roles/README, Common Contract adds the assigned role file. |
| agent-roles, Common Contract: return evidence, checks, changed paths, findings, decisions | Moved unchanged | roles/README, Common Contract. |
| agent-roles, Common Contract: missing authority or capacity and no unauthorized redelegation | Moved unchanged | roles/README, Common Contract. |
| agent-roles, Researcher: dated sources, uncertainty, inference, no design approval | Moved | roles/researcher, primary evidence and promotion limits. |
| agent-roles, Consultant: challenge premises, simpler alternatives, engineer choices | Moved | roles/consultant, alternatives and authority limits. |
| agent-roles, Test author: owner-derived expectations and intended failure | Moved and extended | roles/test-author; test strategy owns detailed proof and disposition rules. |
| agent-roles, Builder: assigned paths, checks, unrelated work | Moved and extended | roles/builder; setup-only permission does not alter assertion semantics. |
| agent-roles, Writer: preserve meaning, criteria mapping, another context's comparison | Moved and extended | roles/writer; documentation governance retains the acceptance requirement. |
| agent-roles, Reviewer: artifacts, surrounding consumers, no editing | Moved and extended | roles/reviewer, including severity and testing review questions. |
| agent-roles, Auditor: trace fidelity and no authority from reports | Moved and extended | roles/auditor, including non-target complexity observations. |
| agent-roles, Independence: self-review, freshness, co-authorship, actionable findings, limits | Moved unchanged | roles/README, Independence. |
| change-execution, Choose A Proportionate Method: source intent and append changes | Clarified | Same heading explicitly forbids rewriting recorded intent revisions. |
| change-execution, Execute The Next Useful Step: reframe and fresh view | Extended | Same heading names commitment, pause/resume, compaction, and replacement triggers without a timer. |
| change-execution, Delegate By Responsibility: role discovery | Relinked | roles/README is the sole role index. |
| change-execution, Delegate By Responsibility: both sides and independent test design | Strengthened from source and brief | Same heading requires independent authorship for important implementation and representation changes. |
| change-execution, Retain Decisions And Findings | Retained | Existing ownership, findings, temporary-state, and retention rules remain. |
| change-execution, Close With Evidence: actual artifact and independent acceptance | Retained | Same heading; missing independent review stays explicit. |
| change-execution, Close With Evidence: final checks and accurate status | Extended | Same heading adds raw code and owner counts plus behavior-preserving maintenance evidence. |
| verification opening: evidence and test selection | Split | verification owns evidence quality; specs/test-strategy owns selection and lifecycle. |
| verification, Start With The Expected Result: known regression, probe exception, setup failures | Retained | Same heading; public proof boundaries are linked to test strategy. |
| verification: unit/business versus integration/API examples | Generalized without losing coverage | Test strategy, Choose A Proof Boundary, applies to Rust modules, CLI, MCP, and native UI. |
| verification: characterization before refactor and content-only proof | Retained | Same heading, plus test strategy lifecycle and toolchain checks. |
| verification, Choose The Lowest Sufficient Observation: five observations and effect limits | Retained | Same heading; no new permanent runner is required. |
| verification, Check The Check: independent expected value | Retained and made explicit | Same heading defines Prior and the other oracle falsifiers. |
| verification: finite, repeatable, affordable, directly observed verdict | Expanded from source | Same heading adds classification and Declare, Demote, Convert behavior. |
| verification: success, deliberate failure, timeout, assertion execution | Expanded from source | Same heading adds self-check limits, explicit outcomes, case accounting, and re-admission. |
| verification: mutation evidence and skipped assertions | Extended | Same heading records both mutated and exercised paths, both observations, identity, and date. |
| verification: environment inconvenience cannot weaken a check | Retained | Same heading and Bound Environment And Effects. |
| verification, Bound Environment And Effects: prerequisites, real target, isolation, unrelated state | Extended | Same heading adds pre-run divergence, exact identity, honest skips, and observed readiness. |
| verification: credentials, cleanup, no production claim from simulation | Retained | Same heading; measurement also preserves the first failure and cleanup outcomes. |
| verification, Report Exact Evidence: final surface, commands, directory, result, limits | Retained and extended | Same heading adds exact artifact identity and links final-head timing. |
| engineering-principles, opening: fresh-context contributors | Extended | Same opening adds the falsifiable delegation commitment and useful engineer participation. |
| engineering-principles, Keep Responsibilities Clear: real variants and owner reuse | Extended | Same heading adds explicit necessity, ablation, deep-module judgment, and distinct authorities. |
| engineering-principles, Keep Feedback Useful: metrics are not targets | Extended | Same heading adds two-sided inventory and shrink-only structural ratchets. |
| engineering-principles, Learn Without Adding Ceremony: same before/after observation and characterization | Retained and extended | Same heading links test retirement proof and recurring structural findings. |
| documentation, Assign One Owner: single authority and conflict resolution | Extended | Same heading names role, testing, and rationale responsibilities and forbids projection inversion. |
| documentation, Write A Contract When Needed: five classes and explicit non-applicability | Retained unchanged | Same heading; no new product requirement. |
| documentation, Keep The Corpus Small: one rule, rationale separate, relative links | Retained | Same heading; all current links remain repository-local and resolvable. |
| documentation: substantive rewrite inventory and independent comparison | Strengthened, not waived | Same heading and roles/writer require old locations and actual diff comparison. |
| documentation, Preserve Existing Authority: no automatic demotion; history is evidence | Retained | Same heading; no product contract is rewritten. |
| documentation: English engineering text and localized operator manuals | Retained with explicit source boundary | Same heading identifies canonical English for new translations and retains the root language rule. |
| writing: sentence targets and longer rationale | Clarified from source | Write Clear Instructions exempts rationale only from the two length targets. |
| writing, Preserve Meaning: qualifiers, one rule, examples, defaults, code documentation | Retained and extended | Same heading links the criterion inventory and independent comparison. |
| toolchain: Rust/Cargo via mise, justified tools, first setup procedure | Retained and clarified | Setup And Dependencies separates provisioning from the command surface. |
| toolchain: dependency owner, purpose, license, maintenance, lockfile | Retained and extended | Setup And Dependencies makes same-change lockfile and guidance updates explicit. |
| toolchain: full Cargo test, Clippy, format, packaging, development Python | Retained | Checks And Packaging; docs-only scope does not erase full code-gate commands. |
| toolchain: whitespace and local links | Retained and extended | Checks And Packaging includes anchors, criteria, final-head formatting, and exact not-run reporting. |
| toolchain: loopback, ffmpeg, ffprobe, native and real-model proof limits | Retained and extended | Checks And Packaging declares platform/opt-in limits before execution. |
| toolchain: temporary probes and no unjustified permanent scripts | Retained and extended | Temporary State And Evidence adds invocation isolation and resource cleanup. |
| docs/README, role navigation | Relinked | roles/README; new rows discover test strategy and execution rationale. |
| 20261014 baseline record, old role link | Relinked only | roles/README; historical observations and conclusions are unchanged. |
| docs/agents, product client and knowledge workflow | Unchanged | This is a product operations guide, not a stale role-policy duplicate. |

## Initial Added Rules And Provenance

The October 22 source supplies public-interface units, real owned stores, behavior assertions, shared setup builders, three-way failure triage, independent retirement, and line-ratio review signals. The October 31 source supplies the admitted rebuild corpus and falsifiable delegation premise. Existing source doctrine supplies oracle falsifiers, harness admission, effect boundaries, explicit environments, independent roles, and lossless documentation.

The brief supplies stricter or missing worker rules: independent authorship for important work; two-pass OLD/NEW coupled-test replacement; specification-first ratchets; shrink-only structural debt; two-sided concept inventories; targeted landings and accounted batches; fix-forward quality assurance with S1/S2/S3; exact-candidate bounded smoke; operator-defect and false-positive dispositions; no measured-arm retries; fail-open observers; public-observable thresholds; final-head gates; and explicit mechanism ablation. Do not infer that these exact formulations already existed in OpenKit's inspected owners.

## Necessity And Ablation Of The Documentation Split

| Added structure | Current need | What happens if it is dropped? |
| --- | --- | --- |
| Per-role files | Test authors, builders, reviewers, and writers need different writable boundaries and independence limits. | A single short role table hides the detailed operating obligations; retaining full contracts there would load unrelated roles for each task. |
| Test strategy separate from verification | Test lifecycle and proof boundaries differ from oracle credibility and environment identity. | Recombining them restores a mixed owner and makes shared testing migration harder. |
| Execution rationale | Keep the reasons for adaptive work outside operative rules. | Readers lose the reason for rejecting fixed ceremony, or reasons return to the procedure and obscure it. |
| Adoption decision and this record | Preserve provenance, exclusions, retained criteria, and missing review evidence. | Future synchronization cannot distinguish source parity from supervisor-supplied additions or check rewrite fidelity. |

No executable layer, runner, registry, source module, dependency, product state, CI workflow, or validator is added. Existing cleanly mapped filenames remain; no alias or stale role duplicate is left.

## Initial Verification And Remaining Work

The initial candidate passed `node temp/doctrine-sync/check-docs.cjs`: 51 Markdown files, 184 local links and anchors, and 23 changed documentation paths. The probe's valid-target and valid-anchor controls passed; deliberate missing-target and missing-anchor controls were detected. `git diff --check` and `cargo fmt --all -- --check` also passed. The external evidence report records the repeated gate on final HEAD with exact command results. Cargo compilation, Clippy, tests, native UI, and packaging are not required for this content-only diff and are not claimed as passed. The first bare Cargo format invocation caused mise to provision the declared Rust 1.99.0 toolchain automatically; no repository setup or source file changed. Product and test source lines changed: zero.

At initial closeout, independent criterion comparison and independent acceptance had not run because the task prohibited starting agents. The sole writer's artifact inspection was self-review only. A later independent review requested the changes recorded below; the supervisor accepted those findings. Neither the initial push nor r2 self-checks constitute independent approval.

## R2 Accepted Review Corrections

The supervisor supplied an independent review of the two initial commits with 0 blocking findings, 6 should-fix findings, and 1 note. The supervisor accepted F1–F6/N1 and supplied explicit F2, F5, and F6 rulings. The [supplemental decision](../decisions/2026-11-04-doctrine-review-rulings.md) records their authority, reasons, and alternatives. This section supersedes over-broad scope and mixed-provenance claims in the historical initial mapping, without rewriting what the first execution observed.

The follow-up began by pulling fresh main at `28df447f4d7ddf1415155d9aa71f9f63e0eb3815`, preserving the concurrent worker's workspace, service contracts, and revised Cargo gate. A daemon restart interrupted editing after F1/F2; local and remote HEAD were checked again and still matched that commit. Only those two files were dirty, and no r2 commit or push had occurred. Work resumed without duplicating a push.

| Finding / old criterion | Accepted change and destination | Retained boundary |
| --- | --- | --- |
| F1: recovery could be self-reconciliation; selected triggers were optional | change-execution, Execute The Next Useful Step: require a fresh independent check after recovery events before renewed investment and at a selected meaningful trigger. | Scope remains material long-running work; ordinary commits within an examined route are not triggers. |
| F2: every addition required experimental removal | engineering-principles, Keep Responsibilities Clear; builder; execution rationale: mandatory necessity and reasoned ablation for every new mechanism, with experiments only for uncertain claims or maintenance/deletion. | Preserve capable observation for experimental necessity or safe-removal claims; a blind green suite is insufficient. |
| F3: admission and re-admission omitted lower-layer exception | verification, Check The Check: distinct process/native/artifact/real-use harnesses need admission; ordinary module/in-process contract files do not acquire a stand-in cycle. | Independent oracles and intended failures still apply to ordinary tests. |
| F4: duplicate retirement lost passing, same clause, and boundary qualifiers | test-strategy, Test Authority And Lifecycle; test-author: require an existing passing check of the same owner clause at an equal/lower sufficient boundary. | Independent disposition remains required. |
| F5: two-pass coupled replacement applied universally | test-strategy lifecycle; test-author; root summary: apply local two-pass proof only to still-required behavior during behavior-preserving replacement. Fresh-main product delivery and workspace-record testing summaries now link that lifecycle instead of repeating the broad condition. | Intended changes use fail-OLD/pass-NEW; pure internal retirement uses independent disposition and coverage accounting without invented requirements or duplicate tests. No product behavior changes. |
| F6: clean working diff could inspect no committed changes | toolchain, Checks And Packaging; change-execution, Land And Check The Final Head: record the reviewed base and check its committed range to HEAD; check staged/unstaged changes separately. | Use the actual integrated baseline and re-gate after rebase or content changes. |
| N1: one missing-in-both row mixed accounting and worker cadence | test-strategy, Keep Coverage And Cost Honest; change-execution; external crosswalk: split source accounting invariants from locally generalized cadence and local-batch application. | No source CI or release machinery is imported. |
| Stronger additions could look like equivalent ports | Label local additions in principles, execution, test strategy, verification, root language restriction, and role summaries. | Source-derived necessity judgment, independent disposition, and accounting invariants remain source-derived, not newly missing doctrine. |

Only doctrine documentation changes in r2. No new mechanism, runtime, registry, dependency, source code, or permanent validator is introduced. The existing temporary structural probe is reused with an explicit reviewed-base argument so concurrent source commits do not become this documentation change's scope. Product/runtime checks remain not run for the content-only diff. Exact final rebased HEAD, reviewed base, whitespace checks, repository-wide format check, link results, commit, and remote verification are recorded in the external report. No agent was started and no coordinator was contacted. A separate independent re-review of r2 has not run; the writer does not claim it did.
