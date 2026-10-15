# Offline Preview Libraries

These files are embedded in the Rust GUI binary.
The preview does not load a content delivery network or access the network at runtime.

| File | Version | License | Download source | SHA-256 |
| --- | --- | --- | --- | --- |
| `mermaid.min.js` | 11.12.0 | MIT | [Mermaid npm distribution](https://cdn.jsdelivr.net/npm/mermaid@11.12.0/dist/mermaid.min.js) | `07e37dfa97b337ccc85365d57eddf99b9706f09db3b59b260d0333b23b343c4b` |
| `highlight.min.js` | 11.11.1 | BSD-3-Clause | [highlight.js distribution](https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.11.1/build/highlight.min.js) | `c4a399dd6f488bc97a3546e3476747b3e714c99c57b9473154c6fb8d259b9381` |

Keep the corresponding license files when updating these artifacts.
Validate diagrams and code highlighting after a version change.

## Native Icons

GPUI Kit uses `gpui-component-assets` 0.5.1 at upstream revision `0f0ab35233212f8f3277028995caf0c41e13ee6c`.
Its Lucide icons include Feather-derived shapes; `lucide-LICENSE` retains both upstream ISC and MIT notices.
The notice comes from [Lucide revision 9b10d1d9](https://raw.githubusercontent.com/lucide-icons/lucide/9b10d1d9bb8554f7d4df81cd5bab7433524c3b5b/LICENSE), with SHA-256 `668dcc52803480e0a026b31140a4cae668772663cd764e5991d252eef03f98db`.
The application bundle includes this notice separately from the Rust crate license inventory.
