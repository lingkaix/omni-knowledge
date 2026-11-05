# Native FSEvents Consumer Contract

The accepted [storage contract](../../docs/specs/storage.md#four-tracking-layers) requires actual FSEvents, durable inventory before cursor publication, and bounded catch-up after continuity loss.
This implementation keeps `LocalMount::watch(Budget)`, `HintWatch::poll`, and `acknowledge_reconciliation` compatible.
`monitor` uses that same watch on both native platforms; it does not own a second macOS implementation.
`HintWatch::native_cursor` returns optional observed volume UUID, held root identity, and device-relative FSEvent ID, never read authority or a replay guarantee.
`Store::reconcile_watched` uses the existing actual bounded reconciliation and appends the observed cursor only after the complete inventory checkpoint is synchronized and the watch epoch and current scope still match.
Restart always starts a new stream and requires full bounded enumeration plus known-descendant inspection; retained event IDs do not authorize pruning or suppress catch-up.

Core Foundation and FSEvents allocate opaque internal memory, so a fixed callback buffer cannot bound their whole runtime in the parent.
The macOS watch therefore owns one prepaid finite-lease `omni-fsevents` child, using the existing processing worker resident-set observer and the same parent Account, not a second scheduler or pool.
LocalReadPolicy remains responsible for its own work inside decide; captured native metadata, including exclusion-vector capacity, and the child remain covered by the watch holding.
The helper receives the selected directory as stdin, resolves its no-firmlink physical path, constructs a slash-free device-relative real stream, and sends fixed-size metadata frames through a nonblocking pipe.
Captured exclusions are component-matched with bounded callback work; loss/control flags cannot be filtered away.
The callback retains no paths, allocates no Rust collections, and runs on the helper's synchronous runloop.
RSS observation is sampled enforcement, not a hard physical cap; allocation and syscall overshoot between samples remain possible.
The helper has its own finite deadline and samples its RSS between bounded runloop calls, including when the parent stops polling.
The runtime lease lasts at most the supplied Budget.max_millis and has a finite aggregate callback-work allowance; the caller must rebuild and reconcile after expiry.
The existing processing observer is reused without its unbounded settle helper, because this native owner must distinguish confirmed exit from bounded unknown retention.
The parent releases the holding and exact handles only after confirmed child exit; an unconfirmed bounded stop retains handles and charge rather than treating EOF, stop request, or Drop as quiescence.
Lease expiry, malformed transport, dropped/coalesced/must-scan/root-change/wrapped IDs, and policy or root drift cannot establish quiet continuity.

Build and install `omni-fsevents` beside the calling executable; library tests can supply the Cargo-built helper with `watch_with_helper` on macOS.
A missing helper is a visible native setup failure, not successful FSEvents or implicit polling.
On an actual macOS CI host, build the helper before the native tests:

```sh
cargo build --locked --offline -p knowledge-local --bin omni-fsevents
target="${CARGO_TARGET_DIR:-target}"
# Cargo integration-test callers live in deps, not beside the ordinary debug bin.
install -m 755 "$target/debug/omni-fsevents" "$target/debug/deps/omni-fsevents"
cargo test --locked --offline -p knowledge-local -p knowledge-storage
```

Packaging and startup callers remain separately owned and must include this helper before claiming macOS readiness.
Linux compile and portable protocol tests are not macOS qualification; the native delivery, root-drift, restart, limits, and settlement tests require actual macOS CI.
