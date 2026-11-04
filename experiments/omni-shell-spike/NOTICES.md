# Notices

This spike is not a product distribution. A CI archive still has to carry the license texts it redistributes.

NSH 0.4.0 is Apache License 2.0, Copyright 2026 Longbridge. The terms copied from tag `v0.4.0` are in `vendor/NSH-LICENSE-APACHE-2.0`. Source for that tag is `https://github.com/longbridge/nsh`. The annotated tag object is `06771fff7150e9ae115a0edde2ab764ec4e8454d`. The peeled commit is `e626a8dd8f8d8b277b9507cfd5ac74d0e67020f3`.

The knowledge UI snapshot is Apache License 2.0 from public OpenKit commit `2f031885c45c68f1a5674f4ec94430f5ac831eb6` on `https://github.com/lingkaix/openkit`. The license text is `vendor/openkit-2f031885/LICENSE`. File identities are `vendor/openkit-2f031885-PROVENANCE.json`. No file in that snapshot was rewritten. The host resolves `packages/knowledge-ui/src/index.ts`.

React, React DOM, and react-aria-components are npm dependencies of the WebView host. Their notices are the files those packages install under `ui/node_modules` at build time. They are not copied into this tree.

The fake client and the shell sources in this directory are part of omni-knowledge and use the repository license. They do not change the NSH or OpenKit license terms.

A packaged binary produced by `nsh package` is a compiled work that includes NSH. Ship `NOTICES.md`, `vendor/NSH-LICENSE-APACHE-2.0`, and `vendor/openkit-2f031885/LICENSE` next to that artifact. Do not describe those files as contents of NSH's own tar or app bundle unless a listing of that archive shows them.
