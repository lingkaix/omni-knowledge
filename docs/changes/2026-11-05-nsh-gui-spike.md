# Isolated NSH GUI Spike

## Intent And Owners

The engineer approved an NSH GUI technology spike at 18:20Z on 2026-11-04.
This change is that spike only. It is measurement, not a product GUI and not core acceptance.
The lease is `experiments/omni-shell-spike/**`, `.github/workflows/omni-shell-spike.yml`, and this record.
Core crates, the root Cargo manifests, main CI, and other workers' files stay untouched.
GPUI stays frozen.

## Implementation

The experiment vendors the public OpenKit knowledge-ui package at commit `2f031885c45c68f1a5674f4ec94430f5ac831eb6` without rewriting those bytes.
A native NSH 0.4.0 sidebar lists fixture mounts. A WebView loads the real `KnowledgeBrowser` through a fake `KnowledgeClient`.
The fake store is in-process. It does not listen as `omni serve`.
Image and PDF reads use `contentUrl` on a loopback listener bound to `127.0.0.1`. `importFile` records name, media type, and size.
The page and the shell use a JSON string. Numeric `id` correlates a call. `nodeId` names a page or file. One object cannot use `id` for both, and without that split the unmodified component cannot complete a read.
`nsh.json` omits capabilities so the file dialog stays available. The document is loaded only from the loopback listener.
Linux packaging depends on `nsh-platform-linux` with the `webview` feature. Without it, NSH shows a placeholder instead of WebKitGTK.
The workflow is a separate concurrency group. It does not edit `ci.yml`. Its path filter does not include `docs/changes/`, so a later note about observed CI does not rebuild the spike.

## Necessity And Ablation

The bridge is required because the React package cannot call the Rust fixture without a channel, and NSH's WebView channel is a string.
Removing `nodeId` makes the call id and the node id overwrite each other, so the reply never settles.
Removing the loopback listener leaves `contentUrl` with no bytes that are not also stuffed into `body`.
The WebView host transpiles the snapshot in its own Vite config because the package tsconfig extends a file the snapshot does not include. The package files are not edited.
No plugin framework was added. The menu, dialog, and drop handler are direct NSH calls.

## Local Observations

These observations do not include a native window, a CI runner, or a pass/fail matrix.

- Contract bytes match the evidence export. `client.ts` sha256 `64a1cbbed707fe01aea259f6d42b957ced16c3a1d7d348fd73221dfcd4e20d8c`. `README.md` sha256 `7cf170f4e1f389e6deab02dec8dee39c796929d43d2a4acbd67cd514f3b562d1`.
- `cargo test` of `omni-shell-fake` with Rust 1.99.0: 7 passed, 0 failed.
- `npm run build` of the WebView host: `dist/main.js` is 249202 bytes and contains `knowledge-ui`, `Load earlier`, and `nodeId`. `dist/asset-index.css` contains the package rules.
- `rustfmt --check` on the spike Rust sources passed. Workflow YAML parsed. `bash -n` on the observer passed.
- crates.io serves `nsh-core`, `nsh-base`, `nsh-platform-linux`, `nsh-platform-macos`, and `nsh-platform-windows` at version 0.4.0.
- This host has no WebKitGTK 6.0. The NSH shell was not compiled here. That is a missing host prerequisite, not an NSH result.
- Remote Linux x86_64 and macOS ARM64 builds, messaging, IME, drag-drop, dialogs, and artifact hashes are not observed in this record.

## First Remote Package Attempt

Push `87a622d5f54a2582b1d0e65621c19990ada10180` started run `41892376378`.
The Linux x86_64 job failed while compiling this experiment's build script, before WebKit linked and before any window ran.
`rustc` reported `E0716` at `experiments/omni-shell-spike/shell/build.rs`: `OUT_DIR` was borrowed from a temporary `String`.
That is a compile error in this spike, not an NSH messaging, build, or IME failure, and not a reason to switch to Tauri.
The following commit binds `OUT_DIR` for the life of the script. It does not claim that the corrected package succeeds.

Run `41894360411` at `2c9b61a` reached the shell library and stopped with `E0425`: `input` is not in `nsh_core::prelude`. The constructor is `nsh_core::element::input`. No window ran. The import fix does not claim a CI result.

Run `41896584847` at `3c8b321` packaged a Linux x86_64 ELF and a macOS ARM64 ad-hoc app. Each Linux observe mode then stopped with `Trace/breakpoint trap`, and the binary log was not in the job log. The following commit prints that log from the ephemeral observer, starts a private session bus there, and sets `WEBKIT_DISABLE_COMPOSITING_MODE=1` only in that observer. It does not claim a messaging, IME, drop, or dialog result.
