# tangoAW2 on iPhone and iPad

The iOS app is the desktop app (`tango/`, iced on wgpu/Metal, mGBA, the
same lobby and rollback netplay) built for `aarch64-apple-ios`, with a
little iOS glue. Same wire protocol and version as the desktop builds, so
iOS and desktop players match each other.

## Building

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
python3 -m pip install pillow          # the icon
ios/build.sh                           # dist/tangoaw2-ios.ipa
ios/build.sh --sim                     # dist/tangoAW2-sim.app (PROFILE=dev for a quick debug build)
ios/cargo.sh check --bin tango --target aarch64-apple-ios
```

`ios/build.sh` builds the binary, renders the icon with `actool`, writes
Info.plist (`ios/bundle.py`, version from `tango/Cargo.toml`), ad-hoc signs
and zips `Payload/tangoAW2.app`. Everything is linked statically: no
embedded frameworks, no entitlements, so AltStore / SideStore / Sideloadly
can re-sign it with a free Apple ID. Minimum iOS 16.

`ios/cargo.sh` runs cargo with three dependency fixes from `ios/deps.py`,
applied through `--config patch...` to copies under `target/ios-deps/` (the
desktop build and Cargo.lock never see them):

- iced_winit: winit's `modifier_supplement` does not exist on iOS; the
  window is the screen; touches become mouse events (`ios/touch_mouse.rs`).
- mgba-sys: its build script rejects unknown target OSes.
- libdatachannel-sys: CMake's iOS mode only searches the SDK, so it cannot
  find the Mbed TLS the same script builds.

## What is different on iOS

All of it is `cfg(target_os = "ios")`; the desktop build is unchanged.

- **Process:** one process, no crash supervisor (`crash-handler` /
  `minidumper` are desktop only). The log goes to the console and to
  `Documents/logs/`.
- **Files:** the data folder is the app's Documents, shown in the Files app
  as *On My iPhone › tangoAW2* (`UIFileSharingEnabled`,
  `LSSupportsOpeningDocumentsInPlace`): `roms/`, `saves/`, `replays/`. The
  welcome screen's button imports files through the Files picker
  (`UIDocumentPickerViewController`); "open folder" actions open the folder
  in Files. Settings live in Library/Application Support. The data folder
  cannot be moved.
- **Input:** an on-screen GBA controller (`tango/src/platform/ios/touch_pad.rs`)
  in sessions you play (not replays): D-pad, A, B, L, R, Start, Select,
  several fingers at once. Portrait puts the game on top and the controls
  below; landscape puts the controls in columns beside the game. Game
  controllers (MFi, Xbox, PlayStation) and a hardware keyboard are read
  through GameController.framework (`bridge.m`) and go through the normal
  input mapping; the on-screen controls step aside once one is used and
  come back on the next touch. In menus, the first finger is the mouse;
  dragging scrolls.
- **Audio:** cpal's CoreAudio backend, with an `AVAudioSession` in the
  playback category (plays with the silent switch on) and a 5 ms buffer.
- **Netplay:** unchanged (WebRTC through libdatachannel, link codes,
  `/host` and `/connect`). `NSLocalNetworkUsageDescription` covers local
  network play; the screen does not lock while the app is open.
- **Updates:** the updater notices a new `-ios.ipa`; installing it opens
  the release page (the sideloading app installs it).
- **Not on iOS:** replay video export (no ffmpeg), Discord presence, the
  data-folder picker. iPad windowing (Stage Manager, Split View) is off
  (`UIRequiresFullScreen`): winit makes a screen-sized window, which a
  windowed scene would scale rather than resize.

## Testing in the Simulator

The Simulator has no command-line touch input, so the app reads touches
from a file when launched with `TANGOAW2_TOUCH_SCRIPT=<file>` (lines like
`tap X Y`, `down N X Y`, `up N X Y`, `wait MS`, in points; see
`ios/touch_mouse.rs`). `TANGOAW2_AUTOSTART=offline` starts a game,
`TANGOAW2_AUTOSTART=link:/connect 127.0.0.1` joins a desktop copy started
with `TANGOAW2_AUTOSTART=link:/host` and readies up, and
`TANGOAW2_ORIENTATION=landscape` turns an iPhone Simulator. Pass them as
`SIMCTL_CHILD_<name>` to `xcrun simctl launch`.
