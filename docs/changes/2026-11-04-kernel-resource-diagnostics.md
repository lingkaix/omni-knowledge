# Kernel startup resource diagnostics

## Necessity

The actual macOS run at `6a52d33` refused all standalone kernel faces before input with a shared `process resource enforcement failed` error. That proves refusal, not which resource or native errno caused it. A separate prerequisite probe cannot identify the actual executable's failed call or inherited limits.

## Change and boundary

The existing private `set_limit` captures errno immediately after a failed `getrlimit` or `setrlimit`, before diagnostic formatting. Its bounded error adds only the static resource name/native identifier, syscall, numeric errno observation, nominal ceiling and, when obtained, inherited/requested limits. No paths, input, mappings, environment, hostname or source contents are reflected. The error code stays `unavailable`.

The AS2GiB, FSIZE512MiB, CPU calculation, stricter-limit calculations, call order, signal behavior and fail-closed startup-before-input are unchanged. No fallback, resource capability claim, platform test exclusion or product integration is introduced. In particular, Darwin does implement `RLIMIT_AS`; incompatible existing VM mappings are only a hypothesis until native evidence identifies the failure. This diagnostic does not measure virtual map size.

## Alternatives and ablation

Retaining the shared text loses the distinction needed to diagnose the actual subject. Only adding an external probe measures another process, not this call. A resource registry, watchdog, changed ceiling or fallback is unnecessary and would alter policy. Removing these fields restores the observed diagnostic ambiguity; no uncertain performance/mechanism claim requires experimental removal.

## Evidence scope

OLD Linux executable behavior is retained at the qualified `6a52d33` receipts; NEW must retain all seven unchanged face cases and strict Clippy. Actual native diagnostics, repaired native library observations and any owner decision about standalone capability remain pending. Neither error formatting nor Linux success establishes macOS/full-M0 acceptance. The vendored patch ledger lists this diagnostic in the same replayable patch.
The ceilings and fail-closed startup behavior are unchanged.
