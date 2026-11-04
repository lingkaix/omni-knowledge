# Full-Version Contracts And Delivery Record

Status: M0 contract reconciliation; implementation milestones remain separate.
Intent sources: product requirements accepted on 2026-10-14/15 and the full-version design plus overriding supervisor amendments accepted on 2026-11-04.
This record explains changed intent, criterion destinations, necessity/ablation, sequencing, and evidence limits.
It is not a behavioral owner, implementation report, or release approval.
Current behavior is owned by [product](../specs/product.md), [storage](../specs/storage.md), [search](../specs/search.md), [history](../specs/history.md), [processing](../specs/processing.md), [package](../specs/package.md), and [service](../specs/service.md).
No private evidence, personal setup, credentials, or private-library data belongs in the public corpus.

## Accepted Outcome And Overriding Amendments

Deliver the complete folder-based knowledge product once, with internal M0–M8 sequencing rather than a reduced MVP or user checkpoints between required features.
One writable home plus selected reference mounts serves the desktop, CLI, MCP, and headless Linux service without accounts, hand-written configuration, or OpenKit governance.
Preserve the detailed existing desktop experience while replacing obsolete data/cache/freshness/exposure assumptions explicitly.
Only S3/cloud delta adapters, optional OCR/scene segmentation, and speculative native Lance FTS replacement remain outside required feature implementation.
A losing time-boxed Lance vector experiment retains the required exact backend and cannot block delivery.

| Overriding amendment | Owning destination and consequence |
| --- | --- |
| macOS CI now | Product Delivery And Qualification: first workflow has macOS arm64 headless/FSEvents tests, GUI build and unsigned app artifact, alongside Linux. Missing CI is blocked, not deferred to the user. |
| Upstreamable vendor seams | Search One Reused Lexical Kernel: generic byte source, read-only Directory/generation provider, caller budgets and shard planning; no omni concepts or OpenKit edits. |
| Neutral shared crates | Product Shared Crates And Platform Boundaries: `knowledge-core`, `knowledge-processing`, `knowledge-retrieval`, vendored `retrieval-kernel`; only service/CLI/GUI keep omni names. |
| Minimal branches | History Minimal Proposal Branches and service commands: agent proposals, list/diff/human confirm; no general branch administration. |
| Headless signing | History Identity And Signing: system signer or explicit visibly marked unsigned choice at CLI library creation, never silent fallback. |
| One delivery/trunk | Product Delivery And Qualification: final-head targeted landing gates, accounted complete batches, fix forward; contracts precede implementation. |
| Lance nonblocking | Search Time-Boxed LanceDB Experiment: exact first, adopt only on registered evidence and retain exact on loss. |
| Smallest mechanism | Necessity/ablation ledger below: reasoned removal analysis for each mechanism; experimental ablation only when its claim remains uncertain. |
| Headless-first GUI deferral | Product Delivery And Qualification: the engineer's 2026-11-04 amendment defers M7 GUI completion, prioritizes the complete headless service, MCP, and CLI mechanisms, keeps the existing GPUI application building with only service-DTO compatibility adaptations, and leaves deferred desktop criteria owned without shipped-implementation claims. |

## Accepted-Design Requirement Map

Each row names an acceptance criterion, its current owner clause, and a falsifying public observation.
Grouped rows retain every qualifier in the owning clauses rather than copying a second normative specification here.

| ID | Requirement and owning clause | Acceptance/failure observation |
| --- | --- | --- |
| P01 | Product Outcome; storage Visible Authority And Layout | Open unmarked/read-only folders without initialization/account/scope; create defaults through normal persistence, keep unknown files. |
| P02 | Storage Visible Authority And Layout | One chosen pages/wiki root, visible manifest/pointers/inert conventions/journal; ambiguous root/collision cannot silently normalize. |
| P03 | Storage Mount Backend And Identity | Logical raw paths preserve original directory structure; attach/rename mount does not copy multi-TB bytes or change source IDs. |
| P04 | Storage Pages And Source Pointers | Filename source ID, verified-or-null digest, path/file-ID/availability/annotations; preserve frontmatter/tag spelling/extensions; page versus asset ownership stays separate. |
| P05 | Storage Legacy Migration And Acceptance | Inventory every hidden sidecar including orphans; full parsed-link map and no-clobber validation precede install; prose/code/line endings/originals survive; old clients reject version 2. |
| P06 | Storage Mount Backend And Identity | Local/NAS/cloud/Git capabilities return typed availability; no escaping links, share authentication, implicit mounting, or S3 registry. |
| P07 | Storage Mount Backend And Identity | Qualified file-ID continuity preserves rename; equal-digest duplicates stay distinct; cross-mount transfer/confirmed association is explicit; inode reuse/same-name replacement cannot steal annotations. |
| P08 | Storage Observation Journal And Rebuildable State | Per-writer sequence/digest/path/availability/evidence/outcome records survive truncation and report malformed bytes; no body/query/key/argument logging or global-clock claim. |
| P09 | Storage Bounded Jobs And Cache | All derivatives outside every home/mount/sync root; synced OS-cache root requires local alternative; per-library sensitive isolation, identity-complete CAS, atomic object installation and no naked-hash access. |
| P10 | Storage Bounded Jobs And Cache | Pins prevent GC, unknown data survives cleanup, visible budget exhaustion stops work without deleting authority/history, eviction changes coverage. |
| P11 | Storage Four Tracking Layers | Verify-on-read, reliable events, bounded reconciliation and cost tiers all operate; changed originals cannot inherit freshness from vectors. |
| P12 | Storage Four Tracking Layers | FSEvents cursor only after durable inventory; drop/root/volume discontinuity scans; inotify startup/overflow/watch-limit loss reconciles. |
| P13 | Storage Four Tracking Layers | Directory mtime is not descendant proof; stat-first/full-verify choices report restored-stat limits; failed/incomplete/offline scans cannot tombstone. |
| P14 | Storage Four Tracking Layers | Writing/growing files wait for quiet stability; reconnect checks identity; cloud conflict copies remain separate; placeholders/unknown residency are unopened without selected authorization. |
| P15 | Storage Bounded Jobs And Cache; service Job | Bounded priority/cancel/resource queue deduplicates version/recipe/dependencies, classifies finite retries, waits offline and supersedes obsolete versions; prior complete generation survives interruption. |
| P16 | Storage Safe Writes And Recovery | Expected digest plus file identity, exclusive creation, displaced-version preservation and validated outcome; lock/stat/unconditional-rename is not CAS. |
| P17 | Storage Safe Writes And Recovery | Competing bytes/proposals remain visible; insufficient primitives refuse replacement; restart inspects actual files/journal/refs and reports indeterminate without blind replay or atomicity fiction. |
| H01 | History Tracked Content And Commit Unit | Text-only one durable operation/commit, multi-file inclusion, no empty job/read commit, journal-only media observation, no media Git/LFS retention claim. |
| H02 | History Identity And Signing | Service-owned human identity and validated agent/tool/session attribution cannot be forged by MCP; real SSH signature/trust differs from display claims; locked signer preserves candidate. |
| H03 | History History Location And Existing Repositories | External durable Git/index/worktrees never enter sync roots; existing repo mode preserves staged/unrelated/media history and refuses unsafe in-sync Git; no hooks/filters/submodules/trust bypass. |
| H04 | History Minimal Proposal Branches | Agent writes propose, accepted head checks base/content, human confirms exact tuple, preserved branches are not concurrent working-tree write permission; no general branch API. |
| H05 | History External Edits And Recovery | Quiet external batches are unknown-origin, not forged agent author; fetched commit signatures survive; interrupted file/ref effects recover same operation without duplicates. |
| H06 | History Git-Server Library And Schedule; service SyncSchedule | Explicit clone/fetch/push/schedule consent, clean fast-forward versus divergence proposal, refusal/offline/auth/unknown result, read-only no-rights and local-only copy; no forced reset/push. |
| R01 | Processing Shared Responsibility; Formats And Bounds | Reuse mature neutral extraction/EPUB/media/provider modules, no GUI/global state; direct text and binary representation coordinates remain distinct; missing parser leaves metadata. |
| R02 | Processing Indexable Unit And Provenance | Original/representation/processor/dependency hashes and real page/slide/sheet/time locators compose with retrieval; PDF parity is observed, never inferred from parser name. |
| R03 | Processing Formats And Bounds; Video And Keyframes | Private bounded tool processes never execute source scripts/macros; real ffmpeg intervals/cancellation/output cleanup; textutil is platform-declared, not assumed on Linux. |
| R04 | Processing Transcripts And ASR | Whisper seconds/milliseconds, invalid/unknown timing, authored correction versus generated ASR, dependency invalidation, no direct WeMM audio or implicit model download. |
| R05 | Processing Embedding Provider | Capability/identity/deadline/input/output bounds; model/package/space/dimensions/preprocessing and finite nonzero vectors validated; loopback external WeMM, deterministic Linux provider not semantic proof. |
| R06 | Search One Reused Lexical Kernel | Exact public pin/downstream build, generic narrow vendor seams and provenance/locks/dictionaries; old competing lexical engine retires only with required behavioral proof. |
| R07 | Search Query Semantics | Canonical byte/case/LF regex, conservative trigram fallback, all-partition scans, pre-limit all-tags versus desktop OR peer tags, no synchronous whole-NAS extraction. |
| R08 | Search File Fusion And Display Evidence | Distinct-file RRF 60, no fragment vote inflation, ranked single-method order, retained useful locations, semantic candidate label, no raw BM25 cross-index comparison. |
| R09 | Search Freshness, Eligibility, And Completeness | Current exposure/source verification at delivery, no leaked excluded names/counts/statistics/distance work; metadata-only unverified discovery remains labelled; top-k and processing/enumeration coverage differ. |
| R10 | Search Disk-Backed Vectors And Generations | Bounded little-endian float32 exact segments, complete identity/row manifests, exact eligibility before distances, whole lexical/vector tuple switch, reader pins and failed-build preservation; no full JSON corpus in RAM. |
| R11 | Search Time-Boxed LanceDB Experiment | Released pinned exact-first comparison at 10k/100k/1M, preregistered hardware/build/disk/cache/RSS, recall/language/performance gates and retirement; no invented result or mandatory ANN adoption. |
| Z01 | Package One Native Format, Three Content Presets | Frozen/backup/migration share ZIP64; selected contents/references/history/journal/index preview, explicit large/sensitive/remote effect consent and honest uncovered digests. |
| Z02 | Package Manifest And Inventory | Canonical manifest snapshot ID differs from physical ZIP/permission; inventory lengths/SHA/methods exclude self-recursion; signature known/unknown/invalid differs from payload integrity. |
| Z03 | Package Hostile Archive Validation | Names/special/encrypted/bomb/unknown-required/header/offset/overlap/ZIP64 validation happens before install; CRC alone cannot authenticate content. |
| Z04 | Package Per-Entry Storage And Direct Search; search Searchable Frozen Generations | STORED media/index/exact vectors, deflated text on demand, held bounded aligned ranges and real read-only Directory; no automatic mmap/Lance/zero-extraction assertion. |
| Z05 | Package Capture, Installation, And Frozen Use | Container/generation pin, entry verification cost, detected drift rejection, private validated atomic no-clobber output, no simultaneous NAS snapshot or in-place frozen edits. |
| Z06 | Package Import And Restore | Verify before destination, new-home default, explicit root remap, inert verified bundle, no unrelated overwrite/deletion sync and no missing-media restoration fiction. |
| Z07 | Package OpenKit v1 Directory/Tar Adapter | Genuine input integrity/core mapping; foreign governance grants nothing; export without actual required host context refuses; changed content cannot keep false revision claims. |
| S01 | Service Commands And Result Mapping; product Shared Crates And Platform Boundaries | All faces use one common service and neutral portable owners; no GUI-private authoritative writes or per-library daemon, default headless graph has no GUI/macOS dependencies. |
| S02 | Service Encoding And Compatibility; Search DTOs; Result | Concrete versioned command/result/error fields, decimal u64 ranges, strict commands/unknown tags, real root sentinel, provenance/coverage, applied query, typed conflict/indeterminate and finite byte frames. |
| S03 | Service Jobs, Mutations, History, And Package DTOs | Operation idempotency differs from request correlation; same-key/different-command refuses; inspect uncertain result before retry; cancel is not rollback. |
| S04 | Service Transports And Exposure | Single-instance owner IPC, direct/proxy stdio, explicit loopback HTTP/MCP, literal host/origin checks, no account/OAuth/public inference/index endpoint; OS loopback is not user identity. |
| S05 | Service Transports And Exposure; Owner Confirmation And Independent Acceptance | MCP default read-only/selected set; model cannot widen roots/exclusions/sensitivity or confirm; sensitive local-only, exclusions/in-flight rechecks, imported expansion pending owner and no query history default. |
| S06 | Service Commands And Result Mapping; package EPUB Publication Boundary | Bounded selected-page/media EPUB plan/export and approved no-clobber destination, same service, readable reopened navigation and no native snapshot-format confusion. |
| D01 | Product Desktop Identity And Setup; Desktop Workspace | Detailed retained workspace/guide/native input/menu/preview/metadata/status requirements remain normative; native human effects are not certified by Linux or source. |
| V01 | Product Delivery And Qualification | First Linux and immediate macOS CI with exact artifacts; arm64 Linux remains unqualified until observed, no unavailable-platform pass; human Mac list contains only actual native/user/provider effects. |
| V02 | Product Delivery And Qualification; repository test strategy | Independent owner-derived public tests, real producer/consumer state, hygienic synthetic fixtures, intended-red versus coupled replacement proof, no weakened/retired failing oracle by builder. |
| V03 | Milestone Exit Gates below; repository change execution/verification | Exact final-head targeted/batch inventory and artifact smoke, independent strict-risk review, harness PASS/FAIL/TIMEOUT admission when declared, retained uncertainty/cleanup. |

## Legacy Criterion Disposition

This is an accepted behavior change with retained features, not a claim of source-preserving refactor.
The comparison baseline is the product/storage/search owners before this record's changes.
The old criterion groups below identify moved, retained, merged, or replaced rules and their accepted reason.

| Old owner/location | Disposition and destination |
| --- | --- |
| Product Outcome: Mac/Rust/GPUI/CLI/wiki/foundations | Retained, expanded to Linux/common service in product Outcome; original snapshots and third-party provenance retained. |
| Product Outcome: required configuration root/nested roots | Required marker replaced by unmarked open plus auto-created data manifest; nested boundary semantics refined in storage Mount Backend And Identity. |
| Product Project License and Desktop Identity And Setup | Retained: Apache-2.0 original-code license, unchanged supplied mark geometry, detailed guide, explicit allowlisted Homebrew Terminal handoff versus success, no implicit model setup, Finder tool discovery. |
| Product Desktop Workspace: compact header/logo/search/chips/input | Retained verbatim except saved/reconciled metadata wording; query alternatives/case, clear behavior, applied-versus-draft identity, nonempty/no-method error, hints and sizing preserved. |
| Product Desktop Workspace: tree/ranked rows/images/locations | Retained; search File Fusion And Display Evidence also retains original 50-file reveal and distinct useful-location/evidence counts. |
| Product Desktop Workspace: guide state/metadata/properties | Retained inspectable record, save state/round-trip, labelled properties/copyable paths, absent dates and independent attribute reads; hidden record changes to visible page/pointer. |
| Product Desktop Workspace: footer/environment/index/root target | Retained compact contextual feedback/actions and no wrong-library refresh; manual-only indexing/uninitialized-root assumptions replaced by open plus bounded tracking, never implicit install/model start. |
| Product Desktop Workspace: context menu/Trash/copy/drop/native input/links/viewers | Retained explicit confirmation, recovery metadata, first-click target/dismissal, no overwrite, Chinese composition/clipboard/word editing and safe native fallback. |
| Product Desktop Workspace: in-folder movable indexes | Replaced by external local cache with library/source identity and honest rebuild; prevents derivative/Git sync corruption without losing movable authored data. |
| Product Source Authority And Effects | Retained originals/authored authority, copy-not-delete import, collision preservation, locators, local processing and no scripts; storage/history now own precise write/recovery effects. |
| Product Implementation Defaults | Required marker/hidden metadata/in-home cache/manual-only search replaced explicitly by storage/history/search/service owners; reusable scoring, complete publication and lexical independence retained. |
| Product Acceptance Examples | Retained all examples and expanded generic Git/cloud/NAS/frozen/headless/restart stories; no personal data requirement. |
| Storage Authority: version 1/sidecars/path identity | Version-1 preview/migration retained; version 2 visible manifest/pointers and qualified source/file identity replace path-only rename behavior; tag spelling/union retained. |
| Storage Authority: nearest marker/jobs/root capture | Nested/root-capture safety retained; unmarked subfolders no longer invented libraries; malformed recognized child marker remains a resolution boundary. |
| Storage Mutations: init/import/reserved name/sidecars/transfer/Trash | No-clobber, source preservation, child exclusions, staged-reservation recovery and Trash retention kept in Safe Writes And Recovery; annotations move out of hidden sidecars. |
| Storage Mutations: cooperative lock and external edits | One home writer retained; stronger expected-digest/displaced-version/proposal safety added explicitly, no claim that a lock excludes external writers. |
| Storage Index Publication: generation/source/model/dependency identity | Retained and expanded to complete tuple, external cache, partitions and real archive providers in search Disk-Backed Vectors And Generations. |
| Storage Index Publication: locks/current/previous/generated inventory | Reader pin/fresh identity/previous-complete/recognized-only cleanup behavior retained; old `.omni/` lock/pointer paths are not the new durable public format. |
| Storage Recovery and Background Status | Authored backups/no unknown deletion/explicit hash limits/incomplete scan honesty retained; automatic tracking supplements manual refresh, paths/cache/model config migrate explicitly. |
| Search Scope And Evidence | Root/generation freshness, locators, semantic candidates, distinct file/location presentation and ranked order retained; verify-on-read strengthens snapshot-only freshness. |
| Search Strategies: grep/BM25/tags/semantic | Canonical kernel tokenizers replace competing analyzer implementation; exact/regex verified fallback, title relevance, tag-before-limit/case intent and explicit provider failure retained. |
| Search Strategies: hybrid/recursive cross-root | File RRF 60/timed evidence retained; labelled groups and declared rank fusion replace uncalibrated raw-score cross-root presentation. |
| Search Unified Desktop Query | Retained every keyword/chip/case/tag/literal/empty/error/applied-settings criterion; public service owns identical request for every consumer. |
| Search Bounds And Coverage | 10,000 distinct-file/500-character limits and bounded extraction/chunks retained with clarified count authority; JSON-resident vectors replaced by disk segments, no latency/quality guarantee inherited from local measurements. |
| Existing GUI/multimodal/agent guides | Retained useful preview/provider/tool/transcript behavior is governed by product/processing/service; historical commands/hidden paths/root overrides do not supersede these accepted owners. Guide projection updates belong to their assigned writers. |

## Necessity And Reasoned Ablation

These mechanisms are the accepted small design, not authorization for further registries/protocols.
A new layer beyond this inventory needs its own present necessity and removal analysis before adoption.
Use an experimental removal only when reasoning/evidence leaves the necessity or safety claim uncertain.

| Mechanism | Present necessity/reuse | What fails if removed; smaller boundary |
| --- | --- | --- |
| Visible manifest/pointers and one text root | Portable mount/library facts and inspectable authored annotations | No durable reopen/annotation identity without them; remove hidden sidecars/duplicate trees instead of adding a registry/setup requirement. |
| Capability adapters and qualified identity | Real local/NAS/cloud/Git differences and move continuity | One pretending filesystem backend lies about watches/residency; equal-hash dedup loses separate annotations; omit S3/delta/plugins. |
| Per-writer visible journal | Digest timeline and interrupted operation/external observation recovery | Old media facts/outcomes disappear without it; one stream per writer avoids global ordering/append consensus. |
| Four tracking layers | Events miss changes, reads need current verification, scans/media have different costs | Dropping a layer permits stale hits, false deletion, restart blindness or mass processing; keep one bounded queue, not a workflow engine. |
| External CAS/cache and rebuildable inventory | No derivatives in sync roots, expensive reuse, bounded metadata lookup | Removing CAS repeats expensive processing; removing DB can remain correct but slow; neither can own annotations or share sensitive namespaces. |
| Expected writes and displaced-version recovery | Noncooperating editors and separate file/ref/cloud effects | Unconditional replacement loses bytes; on insufficient primitives preserve a proposal instead of inventing distributed transactions. |
| Automatic text Git/signing/proposals | Required operation history, attribution, remote text and human-confirmed agents | Removing it loses the requested timeline/identity; keep media out and use system signing/trust, not Notebook governance or new PKI. |
| Pinned vendored kernel seams | Real lexical reuse with otherwise-private mount/archive/budget integration | As-is dependency proves loose files only; keep generic patches/provenance, not a second lexical engine or OpenKit edit. |
| Disk exact vectors and optional Lance | Multimodal retrieval without full JSON RAM scaling | Dropping ANN loses no required semantics; dropping exact loses required packaged direct vectors; experiment decides DB value, not feature scope. |
| Whole-generation identity and reader pins | Lexical/vector/source/model agreement and concurrent readers | Independent latest reads mix incompatible state and GC loses active bytes; reuse one tuple/pointer lifecycle, not a generation service. |
| ZIP64/read-only Directory/range seam | Required large immutable packages and no-extraction STORED search | Unzip-only fails the requested property; omit incompatible optional DB files rather than pretend mmap/search works. |
| One ZIP format/three presets/optional bundle | Frozen, backup and migration differ by selection/retention | Separate formats add converters; no preset promises unselected NAS retention; bundles remain inert until verified. |
| EPUB plan/export over existing plan/job/result | Retained user EPUB publication through the common service | Without an admitted command, GUI/CLI invent a private path; reuse existing bounded plan/job/install machinery rather than another export framework. |
| One service and platform adapters | Same state/logic for all faces, Linux portability and later shared consumers | GUI-private writes or per-library daemons duplicate authority; fixed known adapters suffice without a runtime registry. |
| Owner-only exposure/confirmation | Model cannot widen roots or approve its own proposal | Loopback or a boolean is not human identity; use local peer/launch context and exact proposal tuple, not a login/OAuth system. |

## Milestone Exit Gates

These are implementation acceptance obligations, not results of this documentation task.
The [test strategy](../specs/test-strategy.md) owns independent test authorship, fixtures, dispositions, and proof boundaries.
[Verification](../verification.md) owns harness admission, exact measurement evidence, and environment limits.
[Change execution](../change-execution.md#land-and-check-the-final-head) owns actual final-head timing and complete-suite inventory.

| Milestone | Required exit observation |
| --- | --- |
| M0 contracts/baseline | Accepted generic owners and composable public DTOs; preserved-behavior characterization and baseline failures/dispositions retained; independent intended-red checks named; actual pinned downstream kernel build/lock/provenance proof, not source inference. |
| M1 core/mounts/change/cache | Real public L1/L2 shows offline differs from deleted, rename differs from duplicate, placeholders unopened, four tracking layers, competing writers preserve bytes, restart/idempotency/no-clobber and cache-outside-sync. |
| M2 history/remotes | Real local Git/signing process tests inspect bytes/refs/signatures through one-operation commits, existing staged work, locked signer, proposals/confirmation, divergence/refusal/no-rights and file/ref crash recovery. |
| M3 processing | Real producer-to-retrieval-unit tests preserve direct/representation/time locators and dependency identity; changed transcript/model/recipe invalidates, malformed vectors refuse; actual ffmpeg/ASR proof separate from provider doubles. |
| M4 retrieval/archive seam | Public kernel CJK/Japanese/regex/line parity, distinct-file votes, all-partition completeness, stale withholding, bounded exact vectors, failure-safe tuple publication and archive Directory; time-boxed Lance outcome cannot block. |
| M5 packages/mapping | Actual writer-reader-kernel direct STORED search with extraction disabled, sparse >4 GiB offsets, hostile ZIP refusal, unknown digest/require-complete, incompatible index, capture drift/interruption, safe new-home restore/bundle, honest v1 context refusal and EPUB reopen. |
| M6 daemon/CLI/MCP | Actual headless process/IPC/stdio/HTTP responses prove multi-library exposure/exclusions, root injection/refused writes, sensitive absence, origin checks, changed exposure during delivery, retry/cancel/restart and exact typed results. |
| M7 desktop | Existing detailed GPUI workflow uses frozen DTOs/common service only; macOS arm64 CI builds/tests/FSEvents/unsigned app artifact now; actual human input/preview/Finder/cloud/NAS/model/signing/Gatekeeper effects remain separately unobserved until tested. |
| M8 integrated delivery | Complete deterministic suite/build/lint inventory and exact artifact smoke, restore/sync/recovery stories, independent strict-risk scrutiny and test dispositions, known gaps/cleanup and reproducible user qualification checklist; no MVP feature deferral. |

The 2026-11-04 headless-first amendment defers M7 GUI completion; its exit criteria remain in force when GUI work resumes.
The current delivery keeps the existing GPUI application building and retains the macOS CI GUI-build requirement.
This deferral removes no existing GUI test and changes no non-GUI milestone gate.

Each code landing runs changed/directly related public-seam tests, format/lint/build for its affected crates, then the actual final rebased-head gate.
At integration points, account for `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and a headless release build/artifact smoke.
macOS additionally builds/tests the GUI feature and FSEvents adapter and uploads an unsigned app bundle under the first-workflow requirement.
Do not silently drop declared tests or turn missing prerequisites into a skip; platform/model/user opt-ins remain separately named before execution.
A process harness claiming PASS/FAIL/TIMEOUT must demonstrate those outcomes on stable stand-ins and its actual deciding path before product acceptance.
Do not demand a new test framework or container daemon for ordinary deterministic checks.
Independent scrutiny is required for writes/concurrency, signing/remotes, hostile archives, exposure, and external-cache placement.
Agent dispatch/model selection and lease ownership are delivery coordination, not a public data format or an additional product service.

## M0 Contract Evidence And Residual Work

The initial product/service landing is `28df447f4d7ddf1415155d9aa71f9f63e0eb3815`.
Its targeted final-head checks were whitespace, repository-wide Cargo format, local Markdown references/anchors, JSON examples, and command/result mapping inspection.
Independent early review found missing service EPUB export and an undefined root-browse representation.
This follow-up defines selected bounded EPUB plan/export over existing result machinery and the empty browse-root sentinel, while retaining strict nonempty write paths.
The lead's early composition checks also identified hydrate job kind and schedule observability; service DTOs now represent both explicitly.
The finding is repaired by committed owner changes, not by this narrative declaring review acceptance.

A later independent composition review found that unverified discovery had no read-only path to source bytes: `read_range` required a verified version, while details did not hash and refresh/process required write admission.
The service owner now declares bounded selected-source `verify_source` under read admission, returning the existing Source/ExpectedVersion DTO without relaxing mutation preconditions.
This operation is the existing verify-on-read responsibility made expressible at the public seam, not another processing framework.
Removing it leaves ordinary read-only browse-to-read/preview impossible without authority escalation or an undeclared command.
Its acceptance sequence is unverified resident browse → bounded verification → expected-version range read, with drift, residency, exclusion, and budget refusals.
Independent review of the committed correction and later runtime composition remain required; a documentation example alone proves neither.

No production code or tests are changed by this contract task; changed production/test lines are both zero.
No Cargo compilation, Clippy, product tests, native GUI, model, signing, archive, remote, or throughput result is claimed from documentation checks.
Independent criterion comparison and the implementation M0 baseline/kernel/CI/test duties remain separately owned and cannot be self-certified by this writer.
Root README, documentation-map/guide projections, Cargo workspace/Rust DTO mirrors, and GUI handoff belong to their assigned writers and must align with these owners.
Known residual uncertainty includes native human/platform effects, real-model quality, actual large-library costs, and unexecuted feature gates.
No public claim of full-version implementation or release readiness is made here.
