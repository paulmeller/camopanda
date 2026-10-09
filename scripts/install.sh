#!/bin/sh
set -eu
version=${CAMOPANDA_VERSION:-0.2.0}
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) platform=macos-arm64 ;;
  Linux-x86_64) platform=linux-amd64 ;;
  Linux-aarch64|Linux-arm64) platform=linux-arm64 ;;
  *) echo 'Supported: macOS arm64 and glibc Linux amd64/arm64' >&2; exit 1 ;;
esac
case "$version" in *[!0-9.]*|'') echo 'Invalid release version' >&2; exit 1 ;; esac
install_dir=${CAMOPANDA_INSTALL_DIR:-"$HOME/.local/bin"}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
file=camopanda-$platform.tar.gz
base=https://github.com/paulmeller/camopanda/releases/download/v$version
curl --fail --location --output "$work/$file" "$base/$file"
curl --fail --location --output "$work/SHA256SUMS" "$base/SHA256SUMS"
expected=$(awk -v f="$file" '$2 == f {print $1}' "$work/SHA256SUMS")
test -n "$expected"
if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum "$work/$file" | awk '{print $1}'); else actual=$(shasum -a 256 "$work/$file" | awk '{print $1}'); fi
test "$actual" = "$expected" || { echo 'Release checksum mismatch' >&2; exit 1; }
tar -xzf "$work/$file" -C "$work"
mkdir -p "$install_dir" "$install_dir/../share/camopanda"
for bin in camopanda lightpanda; do install -m 755 "$work/$bin" "$install_dir/$bin"; done
cp -R "$work/licenses" "$work/browser-pins.json" "$install_dir/../share/camopanda/"
"$install_dir/camopanda" version
printf 'Installed to %s. Add this directory to PATH.\n' "$install_dir"
