# Web-Clean Service Boundary

Date: 2026-11-04.
Status: prerelease contract reconciliation; runtime implementation and independent exposure review remain required.
Owner: [service](../specs/service.md#web-status-snapshots).

The current UI-technology planning requires a JSON client boundary, paginated browsing and streaming status without choosing a frontend framework.
The service already owns versioned JSON request/result envelopes, opaque IDs, string-valued large integers, cursor/limit browsing and history, and typed job/coverage states.
Its former polling-only statement did not express streaming status; do not claim MCP's transport name alone supplies that missing service behavior.

Add one bounded HTTP delivery route for the existing `job_status` request and `Response` shape.
The route sends current snapshots as newline-delimited JSON, not a durable event history or a second command catalog.
Single-response HTTP, IPC/stdio envelopes, the existing command union and its fields remain unchanged.
Transport disconnect or budget end never changes a job or implies its success.
A Web client can use the same DTOs and opaque continuations without converting large offsets to JavaScript numbers.
The GUI is lower priority while technology is undecided; this boundary does not introduce a shared-component dependency or authorize new GPUI implementation.

The public service owner fixes the finite rate, connection/frame/duration limits and slow-client behavior.
Every snapshot/chunk uses current normal HTTP read admission; sensitive libraries, excluded details and unexposed jobs remain unavailable.
A stream must not broadcast an owner-IPC status cache to a lower-authority client.
This is part of the existing exposure mechanism, not new authentication, root discovery or a grant carried by a job ID.

Necessity: leaving only polling does not meet the streaming-status requirement.
Reasoned ablation: remove per-delivery admission and revocation can leak later metadata; remove bounds and slow clients can retain unbounded resources.
A replay/event-log/workflow layer is unnecessary because current typed status snapshots meet this requirement and ordinary status queries settle reconnect uncertainty.
Reusing the established JSON response/frame shapes avoids a second status DTO or frontend-specific protocol.

Required later proof composes the actual service producer and Web-facing consumer: complete frame parsing across split HTTP chunks, bounded backpressure/EOF, terminal and nonterminal jobs, capacity refusal, exclusion/sensitivity changes during delivery and non-disclosing guessed-job refusal.
Those are future runtime gates, not passed by this documentation change or a synthetic JSON example.
No GUI/native implementation, runtime event stream, permission widening, legacy-test retirement or completed feature is claimed here.
