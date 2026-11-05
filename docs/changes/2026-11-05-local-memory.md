# Local Working-Memory Reservations

Status: LOCAL-S2-01 correction on the ordinary local read adapter.
Owner: [storage identity and mount backend](../specs/storage.md).
Base: `1dad86ccce4dacca9b12a79dcb98bc32732b7d2a`.
Branch: `omni-local-memory-1dad86c`.
This record does not accept M0, complete M1, or claim READY.
macOS execution is not claimed from this Linux build.

## Scope

`crates/knowledge-local/src/lib.rs` now checks working-memory reservations before the copies and directory stream those reservations cover.
`crates/knowledge-local/src/native.rs` is unchanged.
No manifest, lock, test, example, owner, gate, or Core file changes.
No new dependency, pool, quota service, identity-format limit, or default-budget increase.
`ENTRY_RESERVE` stays 256. `FRAME_RESERVE` stays 512.

## What Changed

Stat, list, and range charge the library and mount strings the adapter clones into a successful snapshot.
When birth or generation is present, the second backend copy and the inode, device, and generation strings are charged too.
A list entry charges its own path and the snapshot path.
A directory frame also charges its prefix and `FRAME_RESERVE` before `read_dir`.
The root frame is refused, as `Partial` / `BudgetExhausted`, when `FRAME_RESERVE` does not fit. An empty directory and a one-byte memory cap do not return `Complete`, and the directory stream is not constructed.
Range subtracts the snapshot charge and the 64-character digest from the call ceiling before building the snapshot.
The hash path then reserves `size_of::<Sha256>()` and gives the retained request and scratch only what remains.
Scratch is dropped before the digest text is allocated.
The requested length still needs one spare byte beyond itself. The whole source file, including bytes outside the returned range, is still charged to `max_bytes`.

Cursor rejection no longer clones the backend identity.
Admission moves the caller’s `outside_scope` vector instead of copying it into another set.
The post-read range check compares the observed fields with the snapshot already held. It does not build a second snapshot.
The walk borrows the live frame prefix instead of cloning it for each name.

A positive budget that cannot hold the next retained copy or frame is `BudgetExhausted`: `Err` for stat and range, `Partial` for list, with entries already accepted kept.
A generous budget that already covered the short identities in the public example is unchanged.

## What Did Not Change

The four review RED cases are not production repairs.
A library or mount mismatch stays `InvalidRequest`.
An offset equal to the verified size stays established EOF after the full digest.
A past-the-end offset stays `InvalidRequest` only when that digest still matches.
The 17-byte memory cap and a 64-byte work cap on an 8-byte path plus a 64-byte file stay insufficient. `metadata_fits` and the whole-source byte charge were not relaxed.
Symlink, special, and non-UTF-8 names stay `Unsupported` and are not followed or hashed.
LOCAL-S2-02 is the independent observer’s worker settlement. This commit does not edit it.

The declared frame reservation is not rustix’s private getdents buffer.
That buffer stays inside the directory stream and is not a second quota.

## Remaining Proof

Independent OLD-fail / NEW-pass coverage of an oversized qualifier under a small finite budget, and of a positive budget below the initial frame, belongs to the test author.
This commit’s Linux evidence is `cargo fmt --all -- --check`, `cargo check -p knowledge-local --locked --offline`, strict package Clippy, and `examples/local_read` printing `ok local-read-smoke`.
No full workspace suite was run.
