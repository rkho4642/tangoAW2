#!/usr/bin/env python3
"""Pieces of the iOS app bundle that ios/build.sh puts together.

  ios/bundle.py icon <out.png>
      The app icon: tango/src/icon.png is a desktop icon (a rounded tile
      on a transparent margin); iOS draws its own rounded mask over a
      full-bleed, opaque square, so take the tile's inside.

  ios/bundle.py plist <out Info.plist> <iphoneos|iphonesimulator> <sdk version> [actool partial plist]
      Info.plist, versioned from tango/Cargo.toml, merged with the icon
      keys actool writes.
"""

import os
import plistlib
import sys
import tomllib

ROOT = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))

BUNDLE_ID = "io.github.rkoh46.tangoaw2"
NAME = "tangoAW2"
MINIMUM_OS = "16.0"


def icon(out):
    from PIL import Image

    src = Image.open(os.path.join(ROOT, "tango", "src", "icon.png")).convert("RGBA")
    # The tile spans ~104..920 with corners of radius ~195; this square
    # lies wholly inside it, chevrons and grid included.
    tile = src.crop((170, 170, 854, 854))
    flat = Image.new("RGB", tile.size, tile.getpixel((tile.width // 2, tile.height - 4))[:3])
    flat.paste(tile, mask=tile.split()[3])
    flat.resize((1024, 1024), Image.LANCZOS).save(out)


def plist(out, platform, sdk_version, partial=None):
    with open(os.path.join(ROOT, "tango", "Cargo.toml"), "rb") as f:
        version = tomllib.load(f)["package"]["version"]
    sim = platform == "iphonesimulator"
    info = {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": NAME,
        "CFBundleExecutable": "tango",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": NAME,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "CFBundleSupportedPlatforms": ["iPhoneSimulator" if sim else "iPhoneOS"],
        "DTPlatformName": platform,
        "DTPlatformVersion": sdk_version,
        "DTSDKName": f"{platform}{sdk_version}",
        "MinimumOSVersion": MINIMUM_OS,
        "LSRequiresIPhoneOS": True,
        "UIDeviceFamily": [1, 2],
        "UIRequiredDeviceCapabilities": ["arm64"],
        "UILaunchScreen": {},
        "UISupportedInterfaceOrientations": [
            "UIInterfaceOrientationPortrait",
            "UIInterfaceOrientationLandscapeLeft",
            "UIInterfaceOrientationLandscapeRight",
        ],
        "UISupportedInterfaceOrientations~ipad": [
            "UIInterfaceOrientationPortrait",
            "UIInterfaceOrientationPortraitUpsideDown",
            "UIInterfaceOrientationLandscapeLeft",
            "UIInterfaceOrientationLandscapeRight",
        ],
        # iOS windows on iPad (Split View, Stage Manager) would scale the
        # app rather than resize it: winit makes a screen-sized window.
        "UIRequiresFullScreen": True,
        "UIStatusBarHidden": True,
        "UIViewControllerBasedStatusBarAppearance": False,
        # iPad trackpads and mice as a pointer, not as touches.
        "UIApplicationSupportsIndirectInputEvents": True,
        # The Documents folder (ROMs, saves, replays) in the Files app:
        # "On My iPhone > tangoAW2", and in Finder over USB.
        "UIFileSharingEnabled": True,
        "LSSupportsOpeningDocumentsInPlace": True,
        # Netplay: direct connections (`/host` / `/connect` to an address)
        # and WebRTC's candidates on the local network.
        "NSLocalNetworkUsageDescription": (
            "tangoAW2 connects to the other player's game for netplay, "
            "including over your local network."
        ),
        "GCSupportsControllerUserInteraction": True,
        "ITSAppUsesNonExemptEncryption": False,
    }
    if partial:
        with open(partial, "rb") as f:
            info.update(plistlib.load(f))
    with open(out, "wb") as f:
        plistlib.dump(info, f)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "icon":
        icon(sys.argv[2])
    elif cmd == "plist":
        plist(*sys.argv[2:])
    else:
        sys.exit(__doc__)
