# Daemon Transport

Date: 2026-11-05.
Status: builder implementation. Native authority and exposure acceptance remain with their owners.

The headless service needs a real stdio, IPC, and loopback HTTP path for the frozen 47-command codec.
Waiting for the authority API would leave that path unbuilt. This crate therefore requires a native handler and starts bounded I/O against it.
The handler receives the decoded request plus the transport facts that arrived with it. Loopback, stdio, Host, and Origin are facts, not a person, a root, or an exposure grant.
No second command schema is introduced. MCP tool names are the codec's `op` strings, minus the owner-only names the service catalog already excludes.
The product binary does not listen until a handler is bound. It does not answer those commands with a permanent not-implemented shell.
Existing `serve-mcp` stays the selected-root adapter with its historical 4 MiB frame. The new service transport uses the codec's 1 MiB control frame. That is a versioned split, not a reclassification of the old ceiling.

Necessity: without these listeners the CLI, MCP, and Web status client have no shared daemon.
Ablation: a hand-rolled request parser would reintroduce request-smuggling choices the locked HTTP/1 stack already refuses; one unbounded status task would let a slow client hold the process.
The process account is printed only when its owner supplies the numbers. This transport does not create a second memory pool or a 50% default.
macOS execution of this daemon is not claimed here.
