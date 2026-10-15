# Omni Knowledge Brand Assets

The engineer supplied the OpenKit mark from `/Users/m5pro/Documents/AI/openkit/apps/web/public/brand/openkit-mark.svg` for reuse.
Its source SHA-256 is `7aa3c6535cb90266da1f0a8c807c45496ce109ff33404eb6200e9dd04990de2a`.
The Omni mark preserves every source path, star, transform, stroke, and the original `-54.79 -55.33 91.93 91.93` viewBox.
Only gradient colors, identifiers, and the accessible label change.
The external source remains unchanged.

| Color | Value |
| --- | --- |
| Light green | `#CDE8BA` |
| Middle green | `#A6D38F` |
| Deep green | `#79AA68` |
| App icon background | `#F3F7EF` |

| Artifact | Use |
| --- | --- |
| [omni-mark.svg](omni-mark.svg) | Transparent vector mark with the original geometry. |
| [omni-mark.png](omni-mark.png) | Transparent 256 × 256 GUI mark. |
| [app-icon.svg](app-icon.svg) | 1024 × 1024 icon source with a rounded cream tile and an optically centered 680-pixel mark. |
| [app-icon.png](app-icon.png) | 1024 × 1024 PNG with transparent outer corners. |
| [app.icns](app.icns) | macOS icon with standard and Retina representations through 1024 pixels. |

The app icon shifts the mark 24 pixels left and up from its geometric center to balance the spiral’s visual weight.
The tile, mark geometry, scale, and colors stay unchanged.

The PNG files use sRGB with an alpha channel and were rendered from the SVG sources with Sharp.
The ICNS file was encoded with Pillow and decoded successfully by macOS `sips` and `iconutil`.
Visual inspection confirmed the original spiral and all three stars, clear padding, and smooth rounded corners.
The bundle script must copy `app.icns` and set `CFBundleIconFile` before signing the app.
