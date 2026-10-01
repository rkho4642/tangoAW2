#!/bin/bash
# Build tangoAW2 for iPhone and iPad.
#
#   ios/build.sh          dist/tangoaw2-ios.ipa, for AltStore / SideStore /
#                         Sideloadly (which sign it with the player's Apple ID)
#   ios/build.sh --sim    dist/tangoAW2-sim.app, for the iOS Simulator
#
# Needs Xcode (for the iOS SDK and actool), the aarch64-apple-ios /
# aarch64-apple-ios-sim Rust targets, cmake, protoc, and Python 3.11+
# with Pillow (for the icon).
#
# The .ipa is ad-hoc signed and carries no entitlements and no embedded
# frameworks (everything is linked statically), so a sideloading app can
# re-sign it with any free Apple ID.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ -z "${DEVELOPER_DIR:-}" ] && [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

sim=0
if [ "${1:-}" = "--sim" ]; then
    sim=1
fi

if [ $sim = 1 ]; then
    target=aarch64-apple-ios-sim
    platform=iphonesimulator
    profile=${PROFILE:-release}
else
    target=aarch64-apple-ios
    platform=iphoneos
    profile=${PROFILE:-release-dist}
fi
sdk_version=$(xcrun --sdk "$platform" --show-sdk-version)

ios/cargo.sh build --bin tango --target "$target" --profile "$profile"

work=target/ios-bundle/$platform
rm -rf "$work"
app=$work/Payload/tangoAW2.app
mkdir -p "$app" "$work/icon/Assets.xcassets/AppIcon.appiconset"

# The binary, without the debug info a release-dist build keeps for
# desktop crash reports (it would double the download).
profile_dir=$profile
[ "$profile" = dev ] && profile_dir=debug
cp "target/$target/$profile_dir/tango" "$app/tango"
if [ $sim = 0 ]; then
    strip -S -x "$app/tango"
fi

# Icon: one 1024 px image; actool renders every size iOS asks for.
python3 ios/bundle.py icon "$work/icon/Assets.xcassets/AppIcon.appiconset/icon.png"
cat >"$work/icon/Assets.xcassets/Contents.json" <<'EOF'
{ "info": { "author": "xcode", "version": 1 } }
EOF
cat >"$work/icon/Assets.xcassets/AppIcon.appiconset/Contents.json" <<'EOF'
{
  "images": [
    { "filename": "icon.png", "idiom": "universal", "platform": "ios", "size": "1024x1024" }
  ],
  "info": { "author": "xcode", "version": 1 }
}
EOF
xcrun actool "$work/icon/Assets.xcassets" \
    --compile "$app" \
    --platform "$platform" \
    --minimum-deployment-target 16.0 \
    --target-device iphone --target-device ipad \
    --app-icon AppIcon \
    --output-partial-info-plist "$work/icon/partial.plist" \
    --output-format human-readable-text --notices --warnings >/dev/null

python3 ios/bundle.py plist "$app/Info.plist" "$platform" "$sdk_version" "$work/icon/partial.plist"
plutil -lint "$app/Info.plist" >/dev/null
printf 'APPL????' >"$app/PkgInfo"

# Ad-hoc signature: the Simulator runs it as is; sideloading apps replace
# it with the player's own.
codesign --force --sign - --timestamp=none "$app"

mkdir -p dist
if [ $sim = 1 ]; then
    rm -rf dist/tangoAW2-sim.app
    cp -R "$app" dist/tangoAW2-sim.app
    echo "dist/tangoAW2-sim.app"
else
    rm -f dist/tangoaw2-ios.ipa
    (cd "$work" && zip -qry -X ../../../dist/tangoaw2-ios.ipa Payload)
    echo "dist/tangoaw2-ios.ipa"
fi
