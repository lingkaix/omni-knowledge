# Native UI Icons

The 80 SVG files are unchanged copies of the Lucide-tagged icons in `gpui-component-assets` 0.5.1.
The published crate records GPUI Kit revision `0f0ab35233212f8f3277028995caf0c41e13ee6c`, with source path `crates/assets/assets/icons/`.
The [published crate](https://crates.io/api/v1/crates/gpui-component-assets/0.5.1/download) SHA-256 is `afc6e4c6551a1a12d4e8b69c3e8eba3cef43331c8c87898a0d4d040c78c6865e`.

Retain the complete [Lucide ISC and Feather MIT notice](../vendor/lucide-LICENSE).
That notice comes from [Lucide revision `9b10d1d9bb8554f7d4df81cd5bab7433524c3b5b`](https://raw.githubusercontent.com/lucide-icons/lucide/9b10d1d9bb8554f7d4df81cd5bab7433524c3b5b/LICENSE), before the GPUI assets publication date.

`gui_assets.rs` embeds only these files and exposes their original `icons/<filename>.svg` paths.
The upstream `github.svg`, `resize-corner.svg`, and four `window-*.svg` files are omitted because their separate asset provenance was not resolved for this bundle.
