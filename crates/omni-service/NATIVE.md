# Native Authority And Domain Dispatch

The native modules implement invocation authority separately from the existing version-1 wire codec.
[Service](../../docs/specs/service.md) owns authorization, exposure and transport behavior.
No request ID, source digest, cursor, root path, transport name or parsed boolean creates a grant.

## Actual Public API

Root's compiled [NativeSource/real Claim/delivery guide](../../src/native_service/README.md) publishes the small consumer boundary without waiting for whole M4. `Runtime::native_scope` uses the same catalog/session/approval route; `Permit::with_selected_context` borrows the actual selected FD/read policy/revision, never selected constants. `Delivery::from_native` retains its genuine prepaid permit through actual write settlement; it does not certify caller buffer accounting or a transport writer.

`NativeAuthority::new()` returns the authority and its separate `Owner` capability.
Trusted startup keeps `Owner` out of request handlers and transport state.
`Owner::select(id, File, LocalPolicy)` installs a descriptor-only selection; every use must stay on that descriptor.
`Owner::select_current(id, File, LocalPolicy, CurrentContext)` additionally binds an owner-installed current namespace/marker check. It runs at connection, every permit check, before/after `with_root`, and deferred delivery. The callback must use native facts and must not reenter authority. Never replace it with a decoded ID/digest/approval claim or a constant true closure. Pathname-based domains require this current-context binding in addition to descriptor-confined IO.
`NativeAuthority::connect(&[Selection], TransportFacts)` creates an opaque native agent session from those capabilities and actual unmodified transport observations.
UID equality and explicit stdio origin are necessary observations, not human confirmation.
HTTP additionally requires actual loopback and literal configured Host/Origin admission; sensitive selections never use HTTP.
This API assumes the trusted adapter supplies observations from the actual connection, not request arguments or bearer claims.

`Owner::approve(session, command, &mut impl HumanIntent)` obtains one exact native intention approval.
Only the separate native owner can call this method on the relevant instance.
The installed native intention flow must present the complete effect/proposal/diff and refuse absent/incomplete/stale interaction.
HTTP cannot obtain this capability.
An approval binds the exact command, native session identity and all selected policy revisions, and moves into one dispatch only.
A successful domain handler must still verify current base/version/diff through the storage/history owner API.

`Runtime::dispatch(bytes, session, &mut impl DomainHost)` reserves decoded frame resources from Core before parsing, applies the one codec, authorizes before resolution/effects, invokes the real domain host and rechecks before result release.
`dispatch_approved` additionally consumes a separately issued native approval; there is no confirmation JSON argument.
`RuntimeError` distinguishes codec refusal from shared-account admission failure before a response exists.
`DomainHost::resolve` resolves opaque source/job/plan identities only under the native permit; it cannot discover a root from request data.
`DomainHost::execute` invokes actual registry/storage/history/processing/retrieval/package owner APIs.
`Domain` is an exhaustive hostcall responsibility match over `Command`, not another operation schema or registry.
No blanket success, empty library fiction or 47-operation NotImplemented implementation is supplied.

`Permit::{check,check_library,check_path}` recheck current native selection/identity/policy and exclusions.
`with_root` holds the selected policy revision across a descriptor-confined owner effect; its closure must not reenter the owner/permit or bypass storage's child-root/current-version rules.
Agent text changes remain proposals; a host must not turn ordinary write admission into human authoring.
Retained asynchronous work must retain a cloned permit and stop/cancel cooperatively after revocation.
`Permit::reserve_result(bytes)` grows the SAME actual frame holding **before** result/vector/map/string buffer growth or deep copies. Cloned permits and `Delivery` retain this aggregate charge through actual write settlement. The initial 64 MiB frame envelope is not permission for unbounded shard/keyword output. Refusal must stop construction before allocation; response encoding is not an allocation gate.
`Permit::retain_check(DeliveryCheck)` retains actual native source/container/generation pins with a current-version check. Checks must capture real descriptors/leases, never parsed approval/version labels; they must not capture/reenter the permit or its resource lock. They run at later permit uses and at `Delivery::{check,encode}`. Root installs checks for observed directories, sidecars and source objects; changing a verified source after encoding prevents the next controllable write. M5 can use the same compiled lifetime surface for its concrete container pin once its source adapter is bound.
`Delivery` retains its shared Core charge through actual transport settlement.
The adapter calls `Delivery::check()` immediately before every controllable write/chunk and never sends previously encoded bytes after refusal.
`Delivery::encode()` rechecks and substitutes a non-disclosing `not_exposed` error after revocation.
Already delivered bytes and completed effects cannot be recalled.

`Runtime::status_stream` permits only normal HTTP `job_status` with the normal native policy checks.
The service-wide count is at most 16; snapshots are immediate then at most once per second for at most 30 seconds/32 frames.
A terminal snapshot or error ends the stream; EOF/disconnection/capacity never means job completion, rollback or cancellation.
The transport still owns framing, bounded write timeout, backpressure and actual tasks, and must retain each Delivery until its write settles.

## Necessity And Limits

Without separate native capabilities, parsed root/transport/exposure claims could authorize themselves.
Without revision-bound approvals, a stale or changed intention could authorize effects.
Without a live permit at effects/delivery, revocation would only change the next request while current disclosures continued.
Without one shared Core charge, each transport could create another memory pool or free a live response too early.
The dispatcher needs no network server, approval database, token issuer, alternate DTO catalog or second executor.

The builder tests use actual owned native directories and real browse observations, current exclusions, revocation and native policy inspection.
They do not certify the production domain adapter composition, safe apply/Git/package mechanisms or native macOS transport observations.
The 9ed consequential review returned CHANGES REQUIRED: selected-root drift, aggregate-output lifetime, and native worker settlement/IO. The follow-up mechanisms and owned controls are implemented in this composition checkpoint; fresh consequential qualification is still required. Native activation remains BLOCKED rather than treating ordinary positive tests as review closure.

## Root Library Host

`omni_knowledge::native_service::NativeLibraryService::ordinary_local(kb, policy)` starts/reuses the sole native account and returns the service with separate `Owner` and `Selection` capabilities. `connect`, `dispatch`, and `dispatch_approved` take real native facts/capabilities and return guarded `Delivery` values. The real host implements confined browse, source details/version verification/range reads, native kernel query, selected-library observations, budget status and exposure. It does not use a DTO root as a grant. The host binds its namespace plus full current marker bytes to the selected descriptor. Browse, stat, source/sidecar verification and native snapshot acquisition use descriptor-relative no-follow operations, never replacement-root path enumeration. Native query pins the selected descriptor's actual read lock and current generation across keywords/methods; retained shard hits, merge structures, metadata and DTO copies grow the permit's aggregate holding before allocation and keep it through Delivery. Query shares a live permit/deadline; excluded-scope query/statistics and image query refuse until their qualified adapters exist.

`bind_domains(Box<dyn DomainHost + Send>)` composes owner adapters through actual typed resolution/execution. An owner can claim an operation during resolution; execution is not retried after possible effects. Unbound history, mutation, mount and package operations report typed unsupported, not empty state or success. Legacy library observations explicitly report migration-required/not-initialized/unprobed where appropriate.

`Delivery::encode` also bounds the encoded response, substituting a typed budget-exhausted response instead of sending an oversized body. Optional `Status.resource_budgets` exposes actual effective Core ceilings; unavailable overrides/waiting counts remain null rather than invented zero. Existing response bytes omit the new field when absent.

Transport activation still belongs to the daemon/CLI/MCP owner. Its adapter must preserve each Delivery and recheck every controllable write/chunk; converting it into an unguarded static Response is not a safe integration. Native IPC facts need actual peer identity, never a transport label or decoded token. Full M1/M2/M5 composition, async job funding/settlement and the consequential review remain integration obligations; this host does not claim those operations are complete.
