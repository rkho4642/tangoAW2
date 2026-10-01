#!/bin/bash
# Run cargo for an iOS target with the three dependency fixes from
# ios/deps.py applied, e.g.
#
#   ios/cargo.sh build --bin tango --target aarch64-apple-ios --profile release-dist
#   ios/cargo.sh check --bin tango --target aarch64-apple-ios-sim
#
# The fixes go in through `--config patch...` on this command line only,
# and Cargo.lock is put back afterwards, so the desktop build and the
# committed lock file never see them.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Xcode's SDKs (xcode-select may point at the Command Line Tools, which
# have no iOS SDK). Keep a DEVELOPER_DIR the caller set.
if [ -z "${DEVELOPER_DIR:-}" ] && [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-16.0}"
export CMAKE_POLICY_VERSION_MINIMUM="${CMAKE_POLICY_VERSION_MINIMUM:-3.5}"

patches=()
while IFS= read -r line; do patches+=("$line"); done < <(python3 ios/deps.py)

cp Cargo.lock target/ios-deps/Cargo.lock.orig
restore() { cp target/ios-deps/Cargo.lock.orig Cargo.lock; }
trap restore EXIT

sub="$1"
shift
cargo "$sub" "${patches[@]}" "$@"
