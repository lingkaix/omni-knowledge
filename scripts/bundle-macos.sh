#!/bin/sh
# Build and validate a locally ad-hoc-signed app without publishing it.
set -eu
cd "$(dirname "$0")/.."
cargo build --release --features gui --bin omni-knowledge-gui --bin omni
bundle="dist/Omni Knowledge.app"
if [ -L dist ] || [ -L "$bundle" ]; then
    printf 'Refusing a symbolic link at the bundle destination\n' >&2
    exit 1
fi
mkdir -p dist
stage=$(mktemp -d "$PWD/dist/.omni-bundle.XXXXXX")
previous=""
published=0
# On failure or interruption, restore the prior artifact if publication is incomplete.
# Preserve its backup if restoration fails; remove only this run's private stage.
cleanup() {
    result=$?
    trap - EXIT
    if [ "$result" -ne 0 ] && [ -n "$previous" ]; then
        failed="${previous}-failed"
        if [ "$published" -eq 1 ] && [ -e "$bundle" ] && [ ! -L "$bundle" ]; then
            if [ ! -e "$failed" ] && [ ! -L "$failed" ] && mv "$bundle" "$failed"; then
                printf 'Preserved failed replacement at %s\n' "$failed" >&2
            fi
        fi
        if [ ! -e "$bundle" ] && [ ! -L "$bundle" ]; then
            if mv "$previous" "$bundle"; then
                printf 'Restored previous bundle at %s\n' "$bundle" >&2
            else
                result=1
            fi
        fi
        if [ -e "$previous" ] || [ -L "$previous" ]; then
            printf 'Restore the preserved bundle from %s\n' "$previous" >&2
        fi
    fi
    rm -rf "$stage"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
staged="$stage/Omni Knowledge.app"
mkdir -p "$staged/Contents/MacOS" "$staged/Contents/Resources"
cp target/release/omni-knowledge-gui "$staged/Contents/MacOS/omni-knowledge-gui"
cp target/release/omni "$staged/Contents/MacOS/omni"
cp LICENSE "$staged/Contents/Resources/LICENSE"
cp assets/brand/app.icns "$staged/Contents/Resources/AppIcon.icns"
mkdir -p "$staged/Contents/Resources/Licenses"
cp assets/vendor/mermaid-LICENSE "$staged/Contents/Resources/Licenses/Mermaid-MIT.txt"
cp assets/vendor/highlight-LICENSE "$staged/Contents/Resources/Licenses/highlight-BSD-3-Clause.txt"
cp assets/vendor/lucide-LICENSE "$staged/Contents/Resources/Licenses/Lucide-ISC-Feather-MIT.txt"
cp assets/foundations/OKF-LICENSE "$staged/Contents/Resources/Licenses/Open-Knowledge-Format-Apache-2.0.txt"
# Python is a build-time helper only; the installed application needs no Python runtime.
python3 -B scripts/collect-licenses.py "$staged/Contents/Resources/ThirdParty"
cat > "$staged/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Omni Knowledge</string>
<key>CFBundleDisplayName</key><string>Omni Knowledge</string>
<key>CFBundleIdentifier</key><string>local.omni-knowledge</string>
<key>CFBundleExecutable</key><string>omni-knowledge-gui</string>
<key>CFBundleIconFile</key><string>AppIcon.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>7</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
# The linker signature covers a standalone executable, not its enclosing bundle.
# Sign nested code first, then bind the final plist and resources to the app.
codesign --force --sign - --timestamp=none --identifier local.omni-knowledge.cli "$staged/Contents/MacOS/omni"
codesign --force --sign - --timestamp=none "$staged"
codesign --verify --deep --strict --verbose=2 "$staged"
plutil -lint "$staged/Contents/Info.plist"

# Fresh inodes avoid replacing signed executable contents in an existing app.
# Preserve the previous artifact and any user additions instead of deleting it.
if [ -e "$bundle" ]; then
    previous="dist/Omni Knowledge.previous-${stage##*.}"
    mv "$bundle" "$previous"
fi
# Mark the attempt before moving so an interruption after the rename can restore the prior app.
published=1
if ! mv "$staged" "$bundle"; then
    exit 1
fi
codesign --verify --deep --strict --verbose=2 "$bundle"
printf 'Built %s\n' "$bundle"
if [ -n "$previous" ]; then
    printf 'Preserved previous bundle at %s\n' "$previous"
fi
