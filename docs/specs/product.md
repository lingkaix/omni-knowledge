# Product Requirements

Status: Accepted product requirements; implementation and verification remain separate.
Acceptance sources: product decisions on 2026-10-14 and 2026-10-15, the full-version design with overriding supervisor amendments accepted on 2026-11-04, and the engineer's headless-first delivery amendment accepted on 2026-11-04.
This document owns product scope, retained desktop requirements, delivery boundaries, and acceptance outcomes.
The [service contract](service.md) owns the public wire surface.
The full version is one delivery; the 2026-11-04 headless-first amendment defers its GUI milestone M7.
Its non-GUI requirements stay complete and are not reduced to an MVP.

## Outcome

Build one local knowledge layer with a GPUI desktop on Apple silicon macOS and the same Rust service on headless Linux.
Provide GUI, CLI, owner-only local IPC, stdio MCP, and explicitly enabled loopback HTTP/Streamable-HTTP MCP faces.
Open ordinary unmarked folders without an account, mandatory hand-written configuration, OpenKit owner scope, Policy, Review, or publication receipt.
Create a visible versioned data manifest through ordinary create, attach, or save actions; opening alone must not initialize a folder.
A library has one writable home and zero or more reference mounts; attaching a folder does not copy its originals.
A read-only folder can be attached to a separate writable home.
Initialize folders with an LLM Wiki workflow and Open Knowledge Format concept documents.
Include the two foundational source articles as first knowledge content, with original offline snapshots, linked reference concepts, and retained provenance.
Preserve selected nested library boundaries and labelled recursive results; an unmarked subfolder is not automatically a separate library.
Admit every regular file type, including binary files, into a general metadata search mechanism.
Provide exact tags, fast grep, indexed BM25, and multimodal retrieval.
Provide rendered Markdown and HTML, front matter, Mermaid, syntax highlighting, and EPUB viewing and export.
Support PDF and Office preview and extraction when mature local solutions exist.
Use local WeMM experiments for images and ffmpeg video segments; provide an audio transcription route.
OCR is optional and may be deferred.

## Project License

The engineer selected Apache License 2.0 for this project's original code in the same conversation on 2026-10-14.
The root `LICENSE` and Cargo SPDX declaration record that choice.
Third-party code, model packages, and source articles retain their original licenses and provenance.
This selection does not relicense bundled dependencies or authorize publication.

## Desktop Identity And Setup

The engineer requested a soft-green version of the existing OpenKit mark for the app logo and icon in the desktop-setup follow-up.
Preserve the supplied vector geometry and keep the original source unchanged.
The desktop must include a detailed guide for its features, reading workflow, retrieval modes, and agent interfaces.
The same page shows runtime dependency availability and installation actions.
Use Homebrew installation actions for external tools such as FFmpeg and whisper.cpp instead of including their binaries in the app.
Run an allowlisted installation only after the user selects its clearly labelled button.
Show installation progress, success, and errors without blocking the desktop interface.
The install button opens a fixed Homebrew command in a visible Terminal window, which owns progress and interaction independently of the app.
Report command handoff separately from installation success, and let the user refresh tool availability after Terminal completes.
If Homebrew is absent, show its prerequisite instructions and official installation page.
Speech models and the WeMM model/provider remain explicit local setup; tool installation must not silently download models or change library configuration.
Tool discovery must work after a Finder launch without relying on the interactive shell's PATH.

## Desktop Workspace

The engineer requested a compact workspace in the 2026-10-15 desktop follow-up.
Remove the two brand text lines from the header and use the green mark to return to the main workspace.
Keep one primary search row at the top and preserve room for files and previews.
Put a clear-search action inside the search input; it clears both the query and tag restrictions and restores the live directory tree.
Show compact, independently enabled search-method chips below the single query field.
Use the same comma-separated keywords for full text, literal text, meaning, file names and notes, and exact tags.
Treat ASCII and Chinese commas as separators, discard empty entries, and match any keyword through any enabled method.
Text and tag retrieval ignore case by default; a tag still requires a complete value rather than a prefix.
Keep the enabled methods visible and remove the separate search-tag input and single-mode dropdown.
Reject a nonempty search with no enabled method and explain how to select one.
Distinguish draft search options from the successful search that produced the displayed files; failed searches must not relabel those results.
Use descriptive retrieval labels and compact action icons with hover hints where the action remains clear.
Align each input's visible height with adjacent compact buttons.
Use suitable GPUI Kit components for the desktop's controls and interaction.
Keep knowledge-folder selection and initialization in an on-demand panel because they are infrequent operations.
Place the next-refresh semantic-index option in that panel and explain that it neither starts processing nor indicates service availability.
Keep adding files beside the tree and index refresh beside index status; do not duplicate index refresh in the search toolbar.
Show unfiltered files in a collapsible directory tree, with selectable leaves and visible source identity.
For an active search, preserve the retrieval order in ranked file rows instead of regrouping files by directory.
Show images as one result per file without expandable metadata or visual-vector fragments.
Expand text results only for distinct line locations and recordings only for distinct time ranges.
Do not present unlocated duplicate metadata as additional navigable evidence.
Keep retained evidence counts separate from these useful display locations, and distinguish semantic candidates from exact matches.
Clicking line evidence must reveal and highlight its numbered text location; retain formatted document reading separately.
Preserve search settings, results, and the selected document when opening and closing the guide.
Keep metadata editing available without reserving a large permanent toolbar row.
Save editable metadata through the service without starting a synchronous index build, and show whether the write succeeded.
Report pending retrieval reconciliation separately from the saved authored values.
Present file details with labelled type, readable size, local creation and modification dates, and explicit path-copy actions.
Show an unavailable date honestly instead of substituting another timestamp.
Label editable tags and notes separately from source properties.
Explain that asset annotations are saved with the library without changing original asset bytes; authored page annotations belong to that page's frontmatter.
Identify the saved record's location, make it inspectable, and reload the saved values when the file is selected again.
Explain when reconciliation makes saved tags and notes available to search, and retain an explicit refresh action.
Automatic bounded tracking supplements explicit refresh; it must not imply that every expensive media job starts immediately.
Distinguish unsaved changes, saving, saved values, and failures, including edits that return to their saved value.
Keep preview selection, typing, and metadata inspection responsive while searches or explicit index builds run.
Read selected-file attributes independently of the complete search index.
Keep the bottom status bar to one compact row during normal browsing.
Show the transfer destination and actionable operation feedback in that row; omit routine preview-ready and file-row instruction messages.
Show complete contextual hints only while hovering, and release any temporary hint space when the pointer leaves.
Remove permanent instructional and destination rows below the file tree and duplicate feedback beside the retrieval controls.
Keep status-bar diagnostics readable within its layout instead of floating under the native preview.
Show selected-folder, index, external-tool, and local-model-service status in a compact bottom bar.
Link dependency problems to their installation or repair instructions.
Distinguish open unmarked folders, pending initialization or migration, invalid selections, missing indexes, source changes, and partial processing coverage.
Background status checks do not initialize, install tools, or start a model provider.
The service can run bounded automatic tracking and reuse or start its single-instance daemon under the service contract.
Provide source-file context actions for copying the file, revealing it in Finder, copying absolute or relative paths, and moving it to the macOS Trash.
Right-clicking a tree row must select its actual target and open its menu on the first click, including when another row's menu is open.
Dismiss that menu when the user clicks outside it, including inside the native preview, or presses Escape.
Require an explicit file action and confirmation before moving a source to Trash; preserve its authored metadata for recovery.
Group the current knowledge root's name and location in one bottom-bar target that opens its folder.
Place index freshness beside the index-refresh action at the right end of the bar, separate from library identity.
Clicking a stale or missing-index state for the opened root must start an explicit index refresh, rather than open documentation.
If the edited path identifies a different library or an unopened unmarked folder, direct the user to open it before offering indexing.
Offer initialization separately when durable library data is needed; do not require it merely to browse.
Never refresh the previously opened library by mistake.
Treat search as a clearable filter over the file tree and restore the complete browsing view when the filter is cleared.
Support explicit file drag-and-drop, copy, and paste within the desktop; preserve metadata and reject overwrite collisions.
Store all derivatives and index generations in an app-owned local cache outside every home, mount, and sync folder.
Bind reusable generations to library/source identities instead of absolute home paths; moving a library must not adopt another library's cache.
A cache miss or incompatible generation causes truthful missing coverage or a rebuild, not loss of authored content.
Use native text inputs with consistent focus, Chinese input-method composition, selection, clipboard commands, and standard word-editing shortcuts.
Open explicit Markdown and HTML links through safe routes for document anchors, contained library files, and the system browser.
Provide default-application, Finder, Quick Look, and metadata actions for the selected source.
Show an explicit unsupported-preview message for formats outside the embedded renderer's coverage, including RAF camera files.
Keep unsupported files available through metadata search and native macOS viewers.

## Source Authority And Effects

Original files and user-edited Markdown or metadata remain authoritative.
Extracted text, search indexes, and vectors are derived data and must be rebuildable.
Import copies sources and never deletes the source.
Initialization must preserve existing content and report collisions.
Search results must identify the source and return line numbers or media time ranges where available.
Local processing is the default; no file content leaves the machine without explicit authorization.
Do not execute scripts from imported content.

## Full-Version Owners And Compatibility

The previous required version-1 marker, hidden annotation sidecars, in-home cache, manual-only change detection, and snapshot-only freshness claims are replaced explicitly.
Legacy data must remain readable for a migration preview; migration must retain orphan annotations and reject collisions without destroying old authored files.
Old clients must reject the incompatible new manifest version instead of interpreting it as version 1.
These accepted requirements do not claim that the legacy implementation already supplies them.

| Responsibility | Full-version requirement and owner |
| --- | --- |
| Mutable layout | Visible `omni-knowledge.json`, one `pages/` text root or mapped existing `wiki/`, `sources/<source-id>.md`, inert `AGENTS.md`, ordinary `schema.yaml`, and per-writer `journal/`; storage owns the exact representation. |
| Mounts and identity | Local, OS-mounted NAS, cloud-synced folder with residency/conflict-copy handling, and Git-server working copies; logical `raw/<mount>/<path>`, no multi-TB copy; source ID differs from digest and backend file ID. |
| Change tracking | Verify on read, reliable platform events, stat-first bounded reconciliation, and cost-tiered jobs together; offline never means deleted, and placeholders never cause background hydration. |
| Authored writes | Expected digest/file identity, one home writer, no-clobber creation, competing-version preservation, typed conflict/indeterminate recovery, and read-only originals by default. |
| Cache | Local external content-addressed derivatives, per-library isolation, explicit budgets, pins and safe retirement; generated ASR is derived, corrected transcripts are authored companions. |
| Text history | Automatic text-only Git, one durable text operation per commit, SSH signing or explicitly selected unsigned creation mode, outside-sync Git directories, minimal agent proposals and human merge confirmation. |
| Git-server library | Explicit remote-effect consent, system credential/signing helpers, fetch plus inspected integration and push; no force/reset, silent unsigned fallback, or implied push permission. |
| Processing | Reuse mature extraction/media/provider behavior with bounded private jobs and original/representation locators; no scripts/macros execute and no direct WeMM audio claim. |
| Retrieval | Shared pinned lexical kernel, CJK/Japanese and canonical scans, metadata/exact tags, file-level RRF 60, exact disk-backed vectors, stable generations and explicit coverage; no full JSON-vector corpus in RAM. |
| Frozen packages | One native ZIP/ZIP64 format with frozen, backup, and migration presets, reference-only large mounts, directly searchable compatible STORED indexes, verification and no-clobber restore. |
| Interoperability | Genuine OpenKit v1 directory/tar mapping is a core-content adapter, not native ZIP, shared permission, or fabricated owner/revision; missing host context causes a precise export refusal. |
| Exposure | Selected libraries and exclusions checked before delivery and writes; MCP read-only by default; owner-only administration, sensitive-library restrictions, no model root widening, and no account/OAuth stack. |
| Desktop | Preserve the detailed workspace, preview, file action, guide, environment, input, and accessibility behavior above; all data operations use the same service as CLI/MCP. |

[Storage](storage.md), [history](history.md), [processing](processing.md), [retrieval](search.md), and [package](package.md) specifications refine these responsibilities without reducing their scope.
S3 and cloud delta APIs are later adapter extensions, not required implementations in this delivery.
OCR, scene-change segmentation, and native Lance FTS replacement remain optional experiments.
The time-boxed LanceDB vector experiment is inside retrieval work and never blocks feature completeness; retain exact vectors if it loses.
No OpenKit source change or governed host implementation is part of this delivery.

## Shared Crates And Platform Boundaries

Use a Cargo workspace with shared `knowledge-core`, `knowledge-processing`, `knowledge-retrieval`, and vendored `retrieval-kernel` crates.
`knowledge-core` owns layout, mounts, journal, safe file operations, text Git, native ZIP, and foreign-format mechanics.
`knowledge-processing` depends on core and owns extraction, media/ASR, provider validation, and retrieval units.
`knowledge-retrieval` depends on core, processing interfaces, and the kernel; it owns metadata/tags, vectors, fusion, coverage, and generations.
`omni-service` orchestrates these owners, libraries, jobs, cache, transports, and exposure.
`omni-cli` and `omni-gui` depend on the service client; neither owns another write path, parser, or retrieval engine.
The default Linux CLI/daemon graph must not link GPUI, AppKit, WebKit, Metal, or an embedded model runtime.
Platform variation uses small known adapters, not a generic plugin registry or a daemon per library.
Shared crates accept explicit handles, paths, providers, and budgets and return serializable facts without GUI globals or OpenKit governance.
Vendored kernel seams must be generic and upstreamable; retain the exact upstream revision, notices, dictionaries, lock evidence, and a patch ledger.
Do not keep the old omni lexical engine alongside the vendored engine permanently.

## Delivery And Qualification

M0 through M8 are internal sequencing within one full delivery.
M0 freezes owners and DTOs, characterizes preserved behavior, names intended-red new requirements, and proves the kernel downstream build.
M1 supplies storage/mounts/tracking/cache; M2 history/remote; M3 processing; M4 retrieval/archive reading; M5 packages; M6 service/transports; M7 GUI; M8 integrates all features.
The 2026-11-04 headless-first amendment defers GUI milestone M7 and prioritizes the complete headless service, MCP, and CLI mechanisms in the current delivery.
The current headless delivery does not require GUI completion; the desktop requirements above remain normative owners for the deferred milestone.
Keep the existing GPUI application building; adapt it only where a service DTO change requires compatibility, and add no GUI redesign.
Each milestone needs direct public-seam evidence; a document or compilation result is not implementation acceptance.
Independent test authors derive checks from owners and DTOs, not candidate source.

Use the [test lifecycle](test-strategy.md#test-authority-and-lifecycle) for behavior-preserving replacement, intended behavior changes, and internal-mechanism retirement; this delivery summary does not broaden those proof obligations.

Each landing gates the final rebased HEAD; integrated batches account for the complete deterministic suite and artifact smoke.

The first CI workflow must include Linux x86_64 headless tests, lint, release build, and artifact smoke.
It must also include macOS arm64 headless tests, FSEvents adapter tests, `--features gui` build, and an unsigned `.app` bundle artifact.
Use modest main-push path filters and manual dispatch, not an expansive matrix.
Linux arm64 build/smoke is required when its runner/toolchain is available and must be labelled unqualified until observed.
Missing macOS CI is a blocked requirement, not permission to defer the GUI build to a user.
Native human qualification covers IME, focus/clipboard/drag/drop, Finder/Quick Look, real OneDrive residency, actual NAS semantics, WeMM GPU/ANE, Keychain prompts, and Gatekeeper.
CI and synthetic Linux adapters do not certify these effects.
Use synthetic or licensed fixtures, disposable keys, and local bare Git remotes; never automatically discover personal libraries or credentials.

## Acceptance Examples

Initialize a nonempty folder without overwriting its existing files.
Import an unknown binary file, annotate it, and retrieve it by exact tag and description.
Retrieve English and Chinese Markdown content through BM25 and grep, then show the original file.
Index modified and deleted files without returning removed content after successful reconciliation.
Keep selected parent and child library identity distinct and return labelled library groups through recursive search.
Retrieve an image and a timed video segment with a natural-language query using the local WeMM service.
Create an EPUB with readable navigation and chapters from Markdown and HTML, then reopen it.
Report missing parser or embedding dependencies without claiming content extraction or semantic coverage.
Open and search English, Chinese, and Japanese text without a model, account, OpenKit scope, or hand-written configuration.
Migrate a version-1 library with orphan sidecars and parsed links without losing annotations, code, prose, or original bytes.
Use a sensitive Git-backed text wiki, a cloud-synced team wiki, and a separate home with NAS plus cloud media mounts as generic acceptance setups.
For the Git wiki, preserve unrelated staged work, inspect a signed operation, and confirm an agent proposal without granting model administration.
For the cloud wiki, observe unknown-origin external batches, conflict copies, placeholders, and in-progress files without hydration or competing-version loss.
For the media home, discover metadata before hashing, process selected pinned clips, keep originals in place, and keep cache/history outside sync roots.
Disconnect/reconnect a mount and stop a provider; distinguish unavailable, deleted, stale, pending, partial, and failed coverage.
Create all three package presets, verify signatures and ZIP64 offsets above 4 GiB, query compatible STORED indexes without extraction, and restore to a new home.
Refuse OpenKit v1 export when genuine required host context is missing; never invent foreign publication or ownership.
Run the same headless service through CLI and multiple stdio/HTTP MCP clients; exclusions, read-only defaults, exposure changes, and restart must hold.
After a forced stop, reopen without duplicate commits, false completed jobs, or automatic replay of uncertain writes.
