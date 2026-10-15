//! Fixed, embedded Lucide icons for GPUI controls without unrelated upstream assets.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

/// Serves the verified local SVG subset with the paths expected by GPUI Component 0.5.1.
/// This provider never reads user files, performs network requests, or serves omitted upstream icons.
pub struct GuiAssets;

/// Generates one static inventory and byte lookup from the approved filenames.
macro_rules! icon_assets {
    ($($filename:literal),+ $(,)?) => {
        /// Original component asset paths included in this application.
        const ICON_PATHS: &[&str] = &[$(concat!("icons/", $filename)),+];

        /// Returns immutable SVG bytes for an exact approved path, or None for any other path.
        fn icon_bytes(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $filename) => Some(include_bytes!(concat!("../assets/icons/", $filename))),)+
                _ => None,
            }
        }
    };
}

icon_assets!(
    "a-large-small.svg",
    "arrow-down.svg",
    "arrow-left.svg",
    "arrow-right.svg",
    "arrow-up.svg",
    "asterisk.svg",
    "bell.svg",
    "book-open.svg",
    "bot.svg",
    "building-2.svg",
    "calendar.svg",
    "case-sensitive.svg",
    "chart-pie.svg",
    "check.svg",
    "chevron-down.svg",
    "chevron-left.svg",
    "chevron-right.svg",
    "chevron-up.svg",
    "chevrons-up-down.svg",
    "circle-check.svg",
    "circle-user.svg",
    "circle-x.svg",
    "close.svg",
    "copy.svg",
    "dash.svg",
    "delete.svg",
    "ellipsis-vertical.svg",
    "ellipsis.svg",
    "external-link.svg",
    "eye-off.svg",
    "eye.svg",
    "file.svg",
    "folder-closed.svg",
    "folder-open.svg",
    "folder.svg",
    "frame.svg",
    "gallery-vertical-end.svg",
    "globe.svg",
    "heart-off.svg",
    "heart.svg",
    "inbox.svg",
    "info.svg",
    "inspector.svg",
    "layout-dashboard.svg",
    "loader-circle.svg",
    "loader.svg",
    "map.svg",
    "maximize.svg",
    "menu.svg",
    "minimize.svg",
    "minus.svg",
    "moon.svg",
    "palette.svg",
    "panel-bottom-open.svg",
    "panel-bottom.svg",
    "panel-left-close.svg",
    "panel-left-open.svg",
    "panel-left.svg",
    "panel-right-close.svg",
    "panel-right-open.svg",
    "panel-right.svg",
    "plus.svg",
    "redo-2.svg",
    "redo.svg",
    "replace.svg",
    "search.svg",
    "settings-2.svg",
    "settings.svg",
    "sort-ascending.svg",
    "sort-descending.svg",
    "square-terminal.svg",
    "star-off.svg",
    "star.svg",
    "sun.svg",
    "thumbs-down.svg",
    "thumbs-up.svg",
    "triangle-alert.svg",
    "undo-2.svg",
    "undo.svg",
    "user.svg",
);

impl AssetSource for GuiAssets {
    /// Returns borrowed embedded bytes for an exact icon path; unknown and traversal paths are absent.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(icon_bytes(path).map(Cow::Borrowed))
    }

    /// Lists approved component paths matching the supplied prefix without filesystem access.
    fn list(&self, prefix: &str) -> Result<Vec<SharedString>> {
        Ok(ICON_PATHS
            .iter()
            .filter(|path| path.starts_with(prefix))
            .map(|path| (*path).into())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exposed inventory has the approved count and cannot serve any of the six excluded assets.
    #[test]
    fn excludes_unresolved_icons_and_traversal_paths() {
        let assets = GuiAssets;
        assert_eq!(assets.list("").unwrap().len(), 80);
        for name in [
            "github.svg",
            "resize-corner.svg",
            "window-close.svg",
            "window-maximize.svg",
            "window-minimize.svg",
            "window-restore.svg",
        ] {
            assert!(assets.load(&format!("icons/{name}")).unwrap().is_none());
        }
        assert!(assets.load("../assets/icons/search.svg").unwrap().is_none());
        assert!(assets.list("icons/window-").unwrap().is_empty());
    }

    /// Prefix listing supports component discovery while each exposed asset retains a Lucide source marker.
    #[test]
    fn lists_prefixes_and_serves_lucide_svg_bytes() {
        let assets = GuiAssets;
        assert_eq!(assets.list("icons/chevron-").unwrap().len(), 4);
        for path in assets.list("icons/").unwrap() {
            let bytes = assets.load(path.as_ref()).unwrap().unwrap();
            let svg = std::str::from_utf8(&bytes).unwrap();
            assert!(svg.contains("<svg") && svg.contains("lucide"));
        }
    }
}
