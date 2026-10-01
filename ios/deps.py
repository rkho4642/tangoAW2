#!/usr/bin/env python3
"""Prepare the three dependency fixes the iOS build needs, and print the
`cargo --config` arguments that point the build at them.

None of these is a fork: each is a copy of the exact crate version
Cargo.lock already pins, with a small edit applied, written under
target/ios-deps/ and used through `[patch]` for the iOS build only. The
desktop build never sees them.

* iced_winit: its keyboard conversion uses winit's `modifier_supplement`
  extension, which winit does not provide on iOS. On iOS it takes the
  same path as the browser build (the logical key and the event's text),
  which is what winit's iOS soft keyboard fills in.
* mgba-sys: the build script panics on any target OS it does not name.
  iOS needs no extra system libraries.
* libdatachannel-sys: CMake's iOS cross-compile mode searches only the
  SDK for libraries, so libdatachannel cannot find the Mbed TLS that the
  same build script has just built. Let it search outside the SDK too.

Usage: ios/deps.py  (prints one argument per line)
"""

import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))
OUT = os.path.join(ROOT, "target", "ios-deps")


def metadata():
    out = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT,
    )
    return json.loads(out)


def find(meta, name):
    pkgs = [p for p in meta["packages"] if p["name"] == name]
    if len(pkgs) != 1:
        sys.exit(f"ios/deps.py: expected one {name} in Cargo.lock, found {len(pkgs)}")
    return pkgs[0]


def edit(path, old, new, count=1):
    with open(path) as f:
        text = f.read()
    if text.count(old) != count:
        sys.exit(f"ios/deps.py: {path}: expected {count} of {old!r}, found {text.count(old)}")
    with open(path, "w") as f:
        f.write(text.replace(old, new))


def stage(pkg, name, link=()):
    """Copy a crate's directory to target/ios-deps/<name>, except the
    (large, untouched) entries in `link`, which are symlinked."""
    src = os.path.dirname(pkg["manifest_path"])
    dst = os.path.join(OUT, name)
    if os.path.lexists(dst):
        shutil.rmtree(dst)
    os.makedirs(dst)
    for entry in os.listdir(src):
        s, d = os.path.join(src, entry), os.path.join(dst, entry)
        if entry in link:
            os.symlink(s, d)
        elif os.path.isdir(s):
            shutil.copytree(s, d, symlinks=True)
        else:
            shutil.copy2(s, d)
    return dst


def main():
    meta = metadata()
    args = []

    # iced_winit (crates.io)
    pkg = find(meta, "iced_winit")
    d = stage(pkg, "iced_winit")
    conv = os.path.join(d, "src", "conversion.rs")
    edit(
        conv,
        '#[cfg(not(target_arch = "wasm32"))]\n                {\n                    use winit::platform::modifier_supplement',
        '#[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]\n                {\n                    use winit::platform::modifier_supplement',
    )
    edit(
        conv,
        '#[cfg(not(target_arch = "wasm32"))]\n                {\n                    use crate::core::SmolStr;',
        '#[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]\n                {\n                    use crate::core::SmolStr;',
    )
    edit(
        conv,
        '#[cfg(target_arch = "wasm32")]\n                {\n                    // TODO: Fix inconsistent API on Wasm',
        '#[cfg(any(target_arch = "wasm32", target_os = "ios"))]\n                {\n                    // TODO: Fix inconsistent API on Wasm',
        count=2,
    )
    # The window is the screen on iOS: winit makes a window of any inner
    # size it is given, and the app's desktop size would overflow it.
    edit(
        conv,
        """    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
""",
        """    #[cfg(target_os = "ios")]
    {
        attributes.inner_size = None;
        attributes.min_inner_size = None;
        attributes.max_inner_size = None;
        attributes.position = None;
    }

    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
""",
    )
    # Touches become mouse events (see ios/touch_mouse.rs).
    shutil.copy2(os.path.join(ROOT, "ios", "touch_mouse.rs"), os.path.join(d, "src", "touch_mouse.rs"))
    lib = os.path.join(d, "src", "lib.rs")
    edit(lib, "mod window;\n", 'mod window;\n#[cfg(target_os = "ios")]\nmod touch_mouse;\n')
    edit(
        lib,
        """                        } else {
                            window.state.update(
                                &program,
                                &window.raw,
                                &window_event,
                            );

                            if let Some(event) = conversion::window_event(
                                window_event,
                                window.state.scale_factor(),
                                window.state.modifiers(),
                            ) {
                                events.push((id, event));
                            }
                        }
""",
        """                        } else {
                            #[cfg(target_os = "ios")]
                            let window_events = touch_mouse::convert(
                                window_event,
                                window.mouse_interaction,
                            );
                            #[cfg(not(target_os = "ios"))]
                            let window_events = [window_event];
                            for window_event in window_events {
                            window.state.update(
                                &program,
                                &window.raw,
                                &window_event,
                            );

                            if let Some(event) = conversion::window_event(
                                window_event,
                                window.state.scale_factor(),
                                window.state.modifiers(),
                            ) {
                                events.push((id, event));
                            }
                            }
                        }
""",
    )
    edit(
        lib,
        """                    event::Event::AboutToWait => {
""",
        """                    event::Event::AboutToWait => {
                        #[cfg(target_os = "ios")]
                        if let Some((id, window)) = window_manager.iter_mut().next() {
                            let scale = window.raw.scale_factor();
                            for ev in touch_mouse::scripted(scale) {
                                for window_event in touch_mouse::convert(
                                    ev,
                                    window.mouse_interaction,
                                ) {
                                    window.state.update(
                                        &program,
                                        &window.raw,
                                        &window_event,
                                    );
                                    if let Some(event) = conversion::window_event(
                                        window_event,
                                        window.state.scale_factor(),
                                        window.state.modifiers(),
                                    ) {
                                        events.push((id, event));
                                    }
                                }
                            }
                        }
""",
    )
    # A crates.io package carries a .cargo_vcs_info / checksum that no
    # longer matches once edited; a path dependency ignores both.
    args.append(f'patch.crates-io.iced_winit.path="{d}"')

    # mgba-sys (git)
    pkg = find(meta, "mgba-sys")
    d = stage(pkg, "mgba-sys", link=("mgba",))
    edit(os.path.join(d, "build.rs"), '"linux" => {}', '"linux" | "ios" => {}')
    src = pkg["source"].split("#")[0].removeprefix("git+").split("?")[0]
    args.append(f'patch."{src}".mgba-sys.path="{d}"')

    # libdatachannel-sys (git)
    pkg = find(meta, "libdatachannel-sys")
    d = stage(pkg, "libdatachannel-sys", link=("libdatachannel", "mbedtls"))
    edit(
        os.path.join(d, "build.rs"),
        '    cmake.define("USE_MBEDTLS", "ON");\n',
        '    cmake.define("USE_MBEDTLS", "ON");\n'
        '    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {\n'
        '        for kind in ["LIBRARY", "INCLUDE", "PACKAGE"] {\n'
        '            cmake.define(format!("CMAKE_FIND_ROOT_PATH_MODE_{kind}"), "BOTH");\n'
        '        }\n'
        '    }\n',
    )
    src = pkg["source"].split("#")[0].removeprefix("git+").split("?")[0]
    args.append(f'patch."{src}".libdatachannel-sys.path="{d}"')

    for a in args:
        print("--config")
        print(a)


if __name__ == "__main__":
    main()
