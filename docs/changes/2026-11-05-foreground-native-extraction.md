# Foreground Native Ownership And Funded Extraction

## Request And Authority

The engineer requested the remaining M4 worker settlement repair and actual Root composition with the committed `8532a19c40824ca7d98c030f88cbc138bda4c7bb` processing API.
The reviewed base is `71290207d22a9448f2b342fec08de466c2b0168b`.
The accepted [resource lifetimes](../specs/storage.md#effective-resource-accounting), [processing boundary](../specs/processing.md), and [native consumer API](../../src/native_service/README.md) govern this correction.
Selected-root S1 and aggregate-result S2 remain carried closed; this change does not reopen their owners.
No Core, local, Storage, processing, Package, History, Root main, MCP, or daemon source is changed.
The engineer prohibited helper agents; independent acceptance of this consequential implementation remains with the engineer/lead rather than a new delegated reviewer.

## Repair

The kernel's Worker previously leaked its Child and Resource when collection was unknown, then returned toward `process::exit`.
It now keeps the real owner in foreground controlled shutdown, with bounded attempts through M3's existing public native group/I/O collector.
The original parent/bootstrap, signal, cache and inherited-descriptor scopes remain reachable around that owner.
Finite nonblocking output batches prevent a stopped consumer from hiding cancellation or native observation.
Non-reaping exit observation pins the leader until whole-group and pipe collection; this executable creates exactly one pure worker, so collected `RUSAGE_CHILDREN` supplies its complete short-lived native peak without baseline subtraction.
Unknown collection retains the actual handles and full charge, emits bounded pending diagnostics, and sleeps between attempts.
Removing that foreground retention would again let EOF/error/unwind reach process termination without an owner; replacing only `process::exit` is insufficient.

`NativeProcessing::extract` consumes the actual selected-source carrier and scheduler Claim, calls the public EEE funded-FD API, and retains the complete paid extraction value.
It checks current source, policy and Claim before/after processing and before borrowed reads.
The result exposes the same source for prepaid response construction, then destroys every extraction field before finishing dead costs and settling the actual Claim.
Errors retain native ownership; mismatched dispatch can recover the real Claim without settling another key.
No alternate queue, worker pool, account, parser, or v1 snapshot engine is added.
The new result/error types exist to bind these inseparable source/Claim/value lifetimes; a raw T cannot express that ownership.
The processor fingerprint includes the actual new consuming implementation.

Compatibility extraction/EPUB/preview now prepay a deep copy before destroying the original M3 buffers and finishing their larger opaque-output envelope.
A raw T still has no last-reader callback: only its copied-buffer charge remains until process exit, not a false native completion claim.
Read-only compatibility failures retain the original input FD through actual bounded native collection.
Root unresolved processing errors retain their original leases in a Box with foreground shutdown, not a leaked global.
Unknown generated-write deltas remain charged: M3 artifact lists can include already-reconciled WAV files, so summing paths or inventing zero would be incorrect.
A producer-owned exact unreconciled-inode capability is still needed for that recovery path.

## Evidence And Remaining Work

All gates use a private target and TMPDIR, two build jobs, locked/offline Cargo, and the verified private IPADIC supply.
Raw logs live in the invocation's `../artifacts/`; no other owner's cache or evidence is deleted.
The frozen product fixture is copied byte-for-byte from committed `8532a19` (`main.rs` blob `8ff0f5dbd8521f2632b0fbc49ce388188f3775a7`, `fixture.rs` blob `1517fbbbe356e2a53847194033e9d99029eeddc0`).
No fixture threshold, assertion, or scenario is changed.
An initial exact-candidate native smoke passed; a mistakenly applied `native_` filter left MCP/product unexecuted, so those filtered counts are not acceptance evidence and the unfiltered gates are recorded separately.
The initial full extraction suite passed 11 tests and MCP passed 2 tests.
Frozen product B2 ordinary parsing now starts a real account and passes, but working allocation remains RED: 6,439,214 bytes against the unchanged 4,275,817-byte ceiling; product observations were 9 passed, 1 failed, 0 not run.
A separate diagnostic using only public M3 `extract_file` reproduces exactly 6,439,214 bytes without Root's deep-copy handoff.
Its allocation trace starts with a 4,259,840-byte capture allocation, then input/diagnostic/JSON decoding overlap; this is a real producer-boundary limit, not permission to raise the oracle or edit another owner's body.
The separate 536,870,912-byte RSS ceiling remains unchanged.
Final initial-boundary gates: `cargo test --locked --offline --workspace --all-targets --no-fail-fast -- --test-threads=1 --nocapture` exited 101: 468 outer-harness tests passed, 46 failed, none ignored or filtered.
The failures are 19 stale independently locked Core-consumer setup failures, 24 local fixture InvalidRoot failures, 2 History setup failures, and frozen B2 working allocation.
The TMPDIR used in that run contained a lexical `..`; this is an invocation setup defect for native owners that require canonical absolute fixture roots, not permission to weaken those checks.
The failed result remains retained; subsequent gates must use the canonical private TMPDIR and consume the committed owner fixture/lock prerequisites rather than change their assertions.
Root native/extraction/MCP and kernel controls passed, including actual foreground EOF/output-error/SIGTERM collection and the escaped-I/O recovery test.
Exact controlled teardown observations were owner/worker PIDs `4071806/4071807` (EOF), `4071808/4071809` (output error), and `4071811/4071812` (SIGTERM): original start identities gone after parent collection, test pipe handles closed.
`cargo clippy --locked --offline --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS=-Dwarnings cargo doc --locked --offline --workspace --no-deps`, and `cargo fmt --all -- --check` each exited 0.
The engineer then published main `5571116` and held-processing ABI `d6ae37d16ddbdde3b33086293f2db2a85eaaef1c`; those are the next caller compile boundary, not evidence that this initial boundary already consumes the held EPUB/video API.

## Committed Held-ABI Caller Boundary

The initial coherent repair is committed as `5b2c634`.
After the engineer published the next ABI, the exact public producer commits were consumed as `4ebc8f8`, `80e40d6`, and `860f2c4`; the complete processing subtree is byte-identical to `d6ae37d16ddbdde3b33086293f2db2a85eaaef1c`.
These are immutable prerequisite integrations, not new edits to another owner's implementation.
The only additional Core path is the standalone consumer Cargo.lock copied verbatim from public `5571116` to supply its already-required sysinfo dependency graph.
No Core production source or owner test assertion is changed, no whole lockfile is generated, and the Root lock's package identities are unchanged in this follow-up.

Root now composes the public held EPUB/video calls through `NativeAdmitted<T>` while preserving the original extraction aliases and source/funding/delivery API.
The actual source/Claim, all member sources, private directory/output FDs, owned staging namespace, original typed errors and complete returned holdings remain together through native collection and last-reader destruction.
Namespace/path/member metadata is prepaid from the same Claim before allocation or staging creation.
Private directory mode is established on the newly created owned FD only; an initial real video gate correctly refused the default nonprivate directory mode, which the caller now fixes rather than bypassing the producer check.
Native FD ancestry excludes the selected source root; logical names never reopen filesystem inputs.
All EPUB members retain prepaid real version pins in the final anchor delivery scope, including members from separate native scopes.
Namespace drift refuses retirement with the real FD and credit retained, rather than deleting a replacement directory.
Stopped partials follow the producer's actual typed accounting contract; unknown child/write state is never stringified into successful settlement.

The actual Root held caller regression reads EPUB spine/Unicode and MP4 bytes through generated FDs, observes original source immutability, separate-scope member drift and real Claim/reader credit lifetimes.
It uses the existing real parser worker and FFmpeg, not a replacement renderer, second engine, mock grant or helper executor.
The canonical private TMPDIR and committed standalone lock repair the invocation/fixture setup failures: the four affected owner harnesses pass 61 tests without changing their expectations.
The complete canonical workspace gate then reports 524 passed, 1 failed, 0 ignored, 0 filtered, with no uncompleted harness.
Its sole failure is the unchanged frozen B2 allocation predicate: ordinary B2 passes and the product harness reports 9 passed, 1 failed, 0 not run.
The deliberate observer-admission NOT_RUN controls are separate from those actual product observations and remain retained as evidence.
Workspace strict Clippy, warning-denied rustdoc and formatting pass on this boundary.
The final metadata-estimate refinement prepays native path/temporary-name copies from the actual cache path length, not an arbitrary larger policy.
Affected final-code gates pass all 29 Root library tests; frozen product remains 9 passed/1 failed with exactly the same 6,439,214-byte peak; workspace strict Clippy, warning-denied rustdoc and formatting each exit 0.
Those commands are retained as `final-root-lib.log`, `final-frozen-product.log`, `final-clippy.log`, `final-docs.log`, and `final-fmt.log` in the invocation artifacts.
Final controlled teardown observes EOF owner/worker `349834/349835` start `245004916`, output-error `349838/349840` start `245004918`, and SIGTERM `349849/349850` start `245004921`: collected, test pipe handles closed.
A final invocation executable-process inventory is empty.
Private build/dictionary caches, raw logs, the direct-M3 allocation diagnostic and intentional observer-admission retained fixtures remain; no other owner's cache/evidence or unknown artifact is deleted.
There was no EDQUOT/SIGBUS in these final gates.
The initial imported frozen fixture dominates test lines and remains exact independent public source, rather than new builder-authored acceptance criteria.

## Handoff And Remaining Queue

The critical owned extraction and held staging APIs are publishable independently of remaining face/publication ports.
The SDK creates no second signal runtime: controlled hosts must retain the typed error and actual Claim in their existing job owner, collect/drop before termination, and never call process::exit while unknown work is owned.
Root main/MCP/daemon/GUI transport and publication integration, including native controlled-shutdown hooks, remains with those existing owners.
The public main `5571116` NativeStoreError/HistoryError owned-capability change is not a claim of full Root-on-557 qualification; those callers must retain the typed error and Claim credit rather than stringify and settle.
B2 needs a producer-owned bounded result-transfer correction: even the direct public M3 call exceeds the frozen ceiling before Root's handoff, so no Root truncation, copied private implementation, or oracle waiver is substituted.
The engineer assigned generic Directory/STORED, model-space and funded paged disk vectors/streaming interfaces to a separate nonoverlapping owner.
The lease acknowledgment covers `crates/knowledge-search/**` and kernel `directory.rs`, `model.rs`, `lib.rs`, `store.rs`, `query.rs`; none is edited in this follow-up.
Root glue for those future interfaces starts only after this handoff.
Independent consequential acceptance remains the lead's reserved serial review; producer assertions and these builder gates do not substitute for that acceptance.

Native staging is compiled and observed above; final publication, vector-provider FD consumption and face callers still require their existing owners' migration.
Raw compatibility APIs are not funded native job owners.
Full Root disk vectors, STORED/generic archive consumption, full-corpus streaming, generated-write recovery, and supported native macOS observations remain OPEN.
Linux source/type/tests do not qualify macOS libproc, descriptor effects, or physical RSS there.
Forced SIGKILL is used only to contain the deliberately irrecoverable ECHILD test subject, never as successful product settlement.
