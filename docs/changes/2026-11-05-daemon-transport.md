# Daemon Transport

Date: 2026-11-05.
Status: builder implementation. Native authority and exposure acceptance remain with their owners.

The headless service needs a real stdio, IPC, and loopback HTTP path for the frozen 47-command codec.
Waiting for the authority API would leave that path unbuilt. This crate therefore requires a native handler and starts bounded I/O against it.
The handler receives the decoded request plus the transport facts that arrived with it. Loopback, stdio, Host, and Origin are facts, not a person, a root, or an exposure grant.
No second command schema is introduced. MCP tool names are the codec's `op` strings, minus the owner-only names the service catalog already excludes.
The product `serve --stdio` and `serve --http` listeners bind a selected library through `Account::start`, `NativeAuthority`, and `Runtime`. Exit 3 remains only when that account start fails.
`serve-mcp` keeps protocol 2024-11-05 and its seven tools. Each tool runs inside `DomainHost::execute` after a live permit check. The legacy path does not add a second reserve on top of the runtime frame. Its frame ceiling is the owner 1 MiB control frame.
History commands call the fixed Git API. `history_list` returns the opened head, not a walked log. `confirm_merge` calls `publish` and does not report Git publication as `saved_local`. A missing intent is `not_found`. `sync` does not contact a remote. Package apply and capture are not called. Retrieval does not wrap `KnowledgeBase` search. Storage stays unsupported until a published cache-file adapter is linked. Root init, add, index, search, list, status, metadata, preview, convert, and export run inside the same account. Preview and convert use `extract_file` and finish the returned holdings.
Refresh, process, pin, and hydrate enter the shared queue. A need that fits the whole budget but not current availability stays queued and visible as waiting for memory or storage. A need above the whole budget is `budget_exhausted`. These jobs are not marked completed. The runtime still returns busy when its own 64 MiB frame cannot fit.
This is not authority acceptance and not final M6. Linux IPC uses `SO_PEERCRED`. macOS IPC calls `getpeereid` and fails closed; this host did not run macOS. A uid is not a human grant.

Necessity: without these listeners the CLI, MCP, and Web status client have no shared daemon.
Ablation: a hand-rolled request parser would reintroduce request-smuggling choices the locked HTTP/1 stack already refuses; one unbounded status task would let a slow client hold the process.
Process memory, storage, and queue counts come from `Account::policy` and `Account::status`. This transport does not create a second memory pool or a 50% default. Queued work uses `Account::submit`. It does not start another account.
macOS execution of this daemon is not claimed here.
