# Agent Operations

The CLI and stdio MCP server share the Rust library used by the desktop interface.
Search returns evidence and source locations; it does not synthesize answers or approve source claims.

## Configure A Client

A generic MCP client entry is:

```json
{
  "mcpServers": {
    "omni-knowledge": {
      "command": "/absolute/path/to/omni-knowledge/target/release/omni",
      "args": ["--root", "/absolute/path/to/library", "serve-mcp"]
    }
  }
}
```

Use the client's supported configuration location; this project does not modify client settings automatically.
The server supports newline-delimited JSON-RPC over stdio and the MCP 2024-11-05 tool subset.
See the primary [transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) and [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle) specifications.
No HTTP listener or remote account is required.

## Workflow

1. Read the generated library `AGENTS.md` and navigation.
2. Import sources or create authored concepts without changing raw originals.
3. Set exact tags and descriptions for sources that need metadata context.
4. Reconcile the index after source edits.
5. Search for candidates, then read original or extracted evidence.
6. Write synthesis with source links and update navigation and history.

The `list` tool returns compact metadata without document bodies.
The `read` tool uses the normal 8 MiB extraction ceiling and bounds returned text to 128 KiB on a valid UTF-8 boundary.
It reports truncation and extraction coverage explicitly.
Read larger originals through the agent's file tools after using the returned path.
Use `index` with `embeddings: true` only when the local provider is running.
A failed provider is an error, not evidence of an empty corpus.
JSON output on the CLI is equivalent evidence for agents without MCP support.
CLI and MCP search keep a single mode and required-tag filters; repeated tags require every complete value, ignoring case by default.
The CLI's explicit `--case-sensitive` flag also makes tag filters case-sensitive.
The desktop's method chips instead treat comma-separated keywords, including tag values, as alternatives through any enabled method.
