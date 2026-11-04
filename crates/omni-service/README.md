# Service Wire Codec

This crate owns the first concrete service behavior: bounded version-1 JSON control decoding and encoding.
The [service contract](../../docs/specs/service.md) owns the public shape and semantics.
This crate does not execute operations or implement authorization, backends, transports, or exposure.
A parsed ID, path, mount capability, exposure declaration, or owner-only command is data, never a grant.
The existing application does not use this crate yet.

## Public API

- `decode_request(&[u8]) -> std::result::Result<Request, CodecError>` checks the supplied byte bound before parsing, UTF-8, one JSON text, version, exact command shape, checked scalars, and explicit command bounds.
- `encode_response(&Response) -> std::result::Result<Vec<u8>, CodecError>` produces compact JSON with bounded output growth, no transport delimiter, and no partial result on error.
- `Request`, `Response`, `Command`, `Result`, and the named domain DTOs are public Serde data types with documented fields.
- `Id`, `Digest`, `Time`, `SignedDecimal`, `RelativePath`, and `RootRelativePath` have checked `new` constructors and `as_str` accessors.
- `U64::new(u64)` retains the full integer and emits a decimal string; `get` recovers the exact local integer.
- `UnsignedInteger`, `PositiveInteger`, and `SafeInteger` check numeric integer bounds with `new`; `get` returns the local integer.
- `FiniteNumber::new(f64)` refuses NaN and infinity; its serializer also refuses non-finite values rather than emitting null.

Use `std::result::Result` or import the wire `Result` under another name to avoid confusing the two meanings.
Required nullable fields use `Option`, but decoding still requires their JSON keys.
No field default, query normalization, UUID restriction, root discovery, or consent inference is added.
Boxes on large result payloads and the recursive operation outcome affect Rust storage only, not JSON.
`wire_version` is public integer data; the codec checks that it equals `WIRE_VERSION` (1).
Direct Serde use does not replace the codec's byte, version, command-bound, and response-bound checks.
A fully decoded unsupported envelope returns `CodecError::UnsupportedVersion`; malformed data never proceeds to execution.
Codec and scalar errors contain fixed diagnostics without input bytes, raw serde errors, or source chains.

The codec checks browse/history limits, read/diff lengths, representation line limits, query limits/method selection, enabled schedule intervals, and nonempty EPUB pages.
Response checks also cover bounded snippets/diffs, package format/selection agreement, and non-nested original operation outcomes.
The frame limit bounds other strings and arrays; no unspecified per-field quota is fabricated.
These checks do not establish that a source digest, coverage, continuation, archive entry, signature, model identity, or effect claim is true.
Those domain checks and per-caller filtering remain with their owners.
The codec does not resolve symlinks or paths, verify Base64 content/provenance, or contact a root.

## Local Checks

Use the root toolchain guide to seed a private dictionary cache before Cargo; offline Cargo alone does not constrain build-script downloads.
Keep the invocation's dictionary environment and run focused checks:

```sh
cargo fmt --all -- --check
cargo check -p omni-service --locked --offline
cargo clippy -p omni-service --all-targets --locked --offline -- -D warnings
cargo test -p omni-service --all-targets --locked --offline
cargo test -p omni-service --doc --locked --offline
```

`examples/codec.rs` consumes one bounded request through the public decoder and emits an explicit unsupported response fixture through the public encoder.
It never executes a command or fabricates a successful empty library/backend result.
It is a producer/consumer demonstration, not a daemon, transport adapter, independent acceptance suite, or authorization check.
The builder-authored RustDoc example also demonstrates syntax only.
Independent authors must derive lasting acceptance checks from the service contract and public Rust surface.
The [wire change record](../../docs/changes/2026-11-04-wire-framework.md) records scope, necessity, limits, and outstanding integration proof.
