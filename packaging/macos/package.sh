#!/usr/bin/env bash
# Build and sign MermaidCraft.app for macOS.
#
#   target/macos-package/MermaidCraft.app     the app (CLI inside Contents/MacOS too)
#   target/macos-package/bin/                 mermaidcraft and mermaidcraft-cli
#   target/macos-package/MermaidCraft-<version>-<arch>.dmg   (with --dmg)
#
# Usage: packaging/macos/package.sh [--arch aarch64|x86_64] [--skip-build] [--dmg]
#
# Signing (env): MACOS_SIGN_IDENTITY  codesign identity; empty or "-" = ad-hoc (local use).
#                With a real identity the hardened runtime and --timestamp are used.
#                MACOS_KEYCHAIN       keychain holding the identity (optional)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

ARCH=aarch64
SKIP_BUILD=0
DMG=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --dmg) DMG=1; shift ;;
    -h | --help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  aarch64) TARGET=aarch64-apple-darwin ;;
  x86_64) TARGET=x86_64-apple-darwin ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

export MACOSX_DEPLOYMENT_TARGET=11.0
IDENTITY="${MACOS_SIGN_IDENTITY:-}"
[[ -z "$IDENTITY" ]] && IDENTITY="-"
VERSION="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p && $1=="version"{gsub(/"/,"",$3); print $3; exit}' "$ROOT/Cargo.toml")"
BUILD_SHA="${MERMAIDCRAFT_BUILD_SHA:-$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)}"
WORK="$CARGO_TARGET_DIR/macos-package"
APP="$WORK/MermaidCraft.app"

if [[ "$IDENTITY" = "-" ]]; then
  echo "warning: ad-hoc signing (no MACOS_SIGN_IDENTITY); Gatekeeper will reject the app on other Macs" >&2
fi
echo "==> MermaidCraft $VERSION for macOS ($ARCH), commit $BUILD_SHA, identity: $IDENTITY"

if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && cargo build --release --locked -p mermaidcraft -p mermaidcraft-cli --target "$TARGET")
fi
BIN_DIR="$CARGO_TARGET_DIR/$TARGET/release"

rm -rf "$WORK"
mkdir -p "$WORK/bin" "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN_DIR/mermaidcraft" "$BIN_DIR/mermaidcraft-cli" "$WORK/bin/"
cp "$BIN_DIR/mermaidcraft" "$APP/Contents/MacOS/MermaidCraft"
cp "$BIN_DIR/mermaidcraft-cli" "$APP/Contents/MacOS/mermaidcraft-cli"

# Icon: rendered from assets/icon.svg with the app's own CLI, then iconutil.
ICONSET="$WORK/MermaidCraft.iconset"
mkdir -p "$ICONSET"
"$WORK/bin/mermaidcraft-cli" convert "$ROOT/assets/icon.svg" "$WORK/icon-1024.png"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" "$WORK/icon-1024.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$WORK/icon-1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns -o "$APP/Contents/Resources/MermaidCraft.icns" "$ICONSET"
cp "$ROOT/assets/fonts/OFL.txt" "$APP/Contents/Resources/OFL-fonts.txt"
for lic in "$ROOT"/assets/fonts/OFL-*.txt; do
  cp "$lic" "$APP/Contents/Resources/$(basename "$lic")"
done

sed -e "s/@VERSION@/$VERSION/g" -e "s/@BUILD_SHA@/$BUILD_SHA/g" "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"

sign() {
  local ts=(--timestamp) rt=(--options runtime) kc=()
  if [[ "$IDENTITY" = "-" ]]; then ts=(--timestamp=none); fi
  [[ -n "${MACOS_KEYCHAIN:-}" ]] && kc=(--keychain "$MACOS_KEYCHAIN")
  codesign --force --sign "$IDENTITY" ${kc[@]+"${kc[@]}"} "${ts[@]}" "${rt[@]}" "$@"
}
# Inside out: nested code first, then the bundle (no --deep on the final signature).
sign --identifier ai.storyteller.mermaidcraft-cli "$APP/Contents/MacOS/mermaidcraft-cli"
sign --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/MermaidCraft"
sign --entitlements "$HERE/entitlements.plist" "$APP"
sign --identifier ai.storyteller.mermaidcraft-cli "$WORK/bin/mermaidcraft-cli"
sign "$WORK/bin/mermaidcraft"
codesign --verify --strict --deep --verbose=2 "$APP"

if [[ "$DMG" = 1 ]]; then
  OUT="$WORK/MermaidCraft-$VERSION-$ARCH.dmg"
  STAGE="$WORK/dmg"
  mkdir -p "$STAGE"
  ditto "$APP" "$STAGE/MermaidCraft.app"
  ln -s /Applications "$STAGE/Applications"
  hdiutil makehybrid -hfs -hfs-volume-name "MermaidCraft" -hfs-openfolder "$STAGE" -o "$WORK/raw.dmg" "$STAGE"
  hdiutil convert "$WORK/raw.dmg" -format UDZO -imagekey zlib-level=9 -o "$OUT"
  rm -f "$WORK/raw.dmg"
  sign "$OUT"
  echo "==> $OUT"
fi
"$WORK/bin/mermaidcraft-cli" --version
echo "==> done: $APP"
