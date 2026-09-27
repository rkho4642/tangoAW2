#!/bin/bash
set -euo pipefail

# Cleanup.
function cleanup {
    rm -rf TangoGBA.iconset TangoGBA.app tango_macos_workdir
}
trap cleanup EXIT
cleanup

# Create directory structure.
mkdir TangoGBA.app{,/Contents{,/{MacOS,Resources}}}

# Generate an appropriate Info.plist.
tools/mako_generate.py "$(dirname "${BASH_SOURCE[0]}")/Info.plist.mako" >TangoGBA.app/Contents/Info.plist

# Create icon.
mkdir TangoGBA.iconset
sips -z 16 16 tango/src/icon.png --out TangoGBA.iconset/icon_16x16.png
sips -z 32 32 tango/src/icon.png --out TangoGBA.iconset/icon_16x16@2x.png
sips -z 32 32 tango/src/icon.png --out TangoGBA.iconset/icon_32x32.png
sips -z 64 64 tango/src/icon.png --out TangoGBA.iconset/icon_32x32@2x.png
sips -z 128 128 tango/src/icon.png --out TangoGBA.iconset/icon_128x128.png
sips -z 256 256 tango/src/icon.png --out TangoGBA.iconset/icon_128x128@2x.png
sips -z 256 256 tango/src/icon.png --out TangoGBA.iconset/icon_256x256.png
sips -z 512 512 tango/src/icon.png --out TangoGBA.iconset/icon_256x256@2x.png
sips -z 512 512 tango/src/icon.png --out TangoGBA.iconset/icon_512x512.png
sips -z 1024 1024 tango/src/icon.png --out TangoGBA.iconset/icon_512x512@2x.png
iconutil -c icns TangoGBA.iconset --output TangoGBA.app/Contents/Resources/TangoGBA.icns
rm -rf TangoGBA.iconset

# Build macOS binaries.
cargo build --bin tango --target=aarch64-apple-darwin --profile release-dist
cargo build --bin tango --target=x86_64-apple-darwin --profile release-dist
lipo -create target/{aarch64-apple-darwin,x86_64-apple-darwin}/release-dist/tango -output TangoGBA.app/Contents/MacOS/tango

ffmpeg_version="8.1.2"

mkdir -p tango_macos_workdir
wget -O tango_macos_workdir/ffmpeg-arm64 "https://github.com/tangobattle/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-macos-arm64"
wget -O tango_macos_workdir/ffmpeg-x64 "https://github.com/tangobattle/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-macos-x86_64"
lipo -create tango_macos_workdir/ffmpeg-{arm64,x64} -output TangoGBA.app/Contents/MacOS/ffmpeg
chmod a+x TangoGBA.app/Contents/MacOS/ffmpeg

# Build zip.
mkdir -p dist
python3 -m dmgbuild -s "$(dirname "${BASH_SOURCE[0]}")/dmgbuild.settings.py" TangoGBA dist/tangogba-macos.dmg
rm -rf tango_macos_workdir
