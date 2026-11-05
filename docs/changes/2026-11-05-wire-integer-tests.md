# Safe Integer Token Tests

Date: 2026-11-05.
Status: independent public-codec checks on owner `b7cf97602fbfcf74db3c1788fa06c1153d142d3a`. This record is not a transport proof, an exposure proof, or a resource-enforcement proof.

## Responsibility

These checks add `crates/omni-service/tests/wire_contract/integers.rs` and declare that module from `main.rs`.
[The service contract](../specs/service.md) requires canonical integer-token spelling for JSON safe-integer fields.
Fractional spelling `100.0` and negative zero `-0` return `CodecError::InvalidWireData` before any effect.
The existing exponent refusal in `json_safe_integers_stop_at_the_published_limit` stays verbatim.
The other 104 wire-contract tests stay in place.
This change does not edit production code, owners, fixtures, or budgets.

## Cases

Shared budget fields `max_files` and `max_millis` preserve tokens `0` and `100`.
The same budget keeps `max_bytes` and `max_memory_bytes` as the decimal string `"0"`.
Browse `limit` preserves token `100` inside the published 1 through 1,000 bound.
Representation `start_line` preserves token `100` and the safe endpoint `9007199254740991`.
`100.0` is refused on both budget integers, on browse `limit`, and on `start_line`.
`-0` is refused on both budget integers, where canonical `0` is legal.
`U64` strings, signed-decimal strings, finite scores, frame bounds, and out-of-range integers stay in their existing checks.
Response-only signed integers are not request fields, so this module does not invent a request shape for them.

## Result

`cargo fmt --all -- --check` passed.
`cargo test -p omni-service --all-targets --locked --offline -j 2 -- --list` listed 108 tests and 0 benchmarks, with no ignored test.
`cargo clippy -p omni-service --all-targets --locked --offline -j 2 -- -D warnings` passed.
`cargo test -p omni-service --all-targets --locked --offline -j 2` reported 108 passed, 0 failed, and 0 ignored.
`cargo test -p omni-service --doc --locked --offline -j 2` reported 1 passed and 0 ignored.
Source `#[test]` attributes are 108: the previous 104 plus these 4.
The candidate already accepted the new refusals, so this record does not invent an older failure.
