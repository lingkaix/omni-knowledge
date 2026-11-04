# Omni shell spike

This directory is a measurement of NSH 0.4.0. It is not the product GUI, not a service, and not a core acceptance test.

A native sidebar lists two fixture mounts. A WebView island loads the shared React package `@openkit/knowledge-ui` from the vendored snapshot of OpenKit commit `2f031885c45c68f1a5674f4ec94430f5ac831eb6`. The page talks to a fake `KnowledgeClient` in the spike process. `omni serve` is not used. Mount ids, node ids, and version strings are fixture data.

NSH 0.4.0 is Apache-2.0, Copyright 2026 Longbridge. The OpenKit snapshot is Apache-2.0. See `NOTICES.md` and `vendor/`.

GPUI stays frozen. This experiment does not add a GPUI surface.

## What is real

The sidebar, the menu, the file dialog request, and the WebView are NSH. The React component is the unmodified package. The bridge is a JSON string on `window.nsh.postMessage` and `post_message`. A call uses numeric `id` for correlation and `nodeId` for a page or file, because one JSON object cannot use `id` for both.

The fake store scripts one version conflict. The first read of `p-note` returns `v1`, then the stored version becomes `v2`, so the component's save reports `{kind: version-conflict, currentVersion: v2}`. Image and PDF reads use `contentUrl` on the spike's loopback listener. `importFile` records the name, media type, and size. The host does not copy the `Blob` into a byte array.

`nsh.json` omits `capabilities`. A partial capabilities object would deny the file dialog. The page is loaded only from `127.0.0.1`.

## Build

From `ui/`: `npm ci` and `npm run build`. From `shell/`: `nsh package --target linux` or `nsh package --target macos`. The NSH CLI is 0.4.0 and the crates are pinned to `=0.4.0`. Linux links `nsh-platform-linux` with the `webview` feature so the view is WebKitGTK 6.0 rather than a placeholder. Do not put `crates/nsh-core` in this repository. That path would make `nsh` compile against a checkout instead of crates.io.

`OMNI_SHELL_SPIKE_MODE` selects `probe`, `ime`, `drop`, or `dialog`. Unset stays open. `OMNI_SHELL_SPIKE_EVIDENCE` is the JSON path. The JSON records facts. It does not assign the pass/fail matrix.

The Linux observer is `scripts/linux-observe.sh`. It refuses a display that is already set, starts its own Xvfb, and keeps ibus config under a temporary home. It is for the isolated workflow, not this machine's desktop.

Packaging is unsigned CI. macOS `nsh package` ad-hoc signs the `.app`. The workflow's `.dmg` is a separate `hdiutil` image of that app. Neither is Developer ID or notarized. Notices sit beside the archive. They are not claimed to be inside NSH's tar unless a listing shows them there.
