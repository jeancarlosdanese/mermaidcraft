#!/usr/bin/env bash
# Build and package MermaidCraft for Linux as a plain FHS-style tree:
#
#   dist/release/mermaidcraft-<version>-linux-<arch>.tar.gz   bin/, share/ (desktop file, MIME type, icons, licenses)
#
# Install for the current user: tar xzf <tar.gz> -C ~/.local --strip-components=1
# (craft-update does this and refreshes the desktop, MIME and icon caches), or run with --install.
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats tar] [--install [--prefix DIR]]
#   --install  also extracts the tree into DIR (default ~/.local), points the menu entry at the
#              installed binary and refreshes the desktop, icon and MIME caches.
# Needs: cargo. The Wayland and X11 libraries are loaded at run time; inkscape or ImageMagick
# (magick or convert), when present, add PNG icons (the scalable SVG is always installed).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# Relative paths are taken from where the script was called (cargo runs from $ROOT).
absolute() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$PWD" "$1" ;; esac; }
CARGO_TARGET_DIR="$(absolute "${CARGO_TARGET_DIR:-$ROOT/target}")"
export CARGO_TARGET_DIR
DIST="$(absolute "${DIST:-$ROOT/dist/release}")"
APP_ID=ai.storyteller.mermaidcraft

SKIP_BUILD=0
INSTALL=0
PREFIX_DIR="$HOME/.local"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats)
      [[ "${2:-}" = tar ]] || { echo "only --formats tar is supported" >&2; exit 2; }
      shift 2 ;;
    --install) INSTALL=1; shift ;;
    --prefix) PREFIX_DIR="$(absolute "${2:?--prefix needs a folder}")"; shift 2 ;;
    -h | --help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ARCH="$(uname -m)"
VERSION="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p && $1=="version"{gsub(/"/,"",$3); print $3; exit}' "$ROOT/Cargo.toml")"
[[ -n "$VERSION" ]] || { echo "version not found in [workspace.package] of Cargo.toml" >&2; exit 1; }
BASENAME="mermaidcraft-$VERSION-linux-$ARCH"
echo "==> MermaidCraft $VERSION for Linux $ARCH (tar)"

if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && cargo build --release --locked -p mermaidcraft -p mermaidcraft-cli)
fi
BIN="$CARGO_TARGET_DIR/release"
STAGE="$CARGO_TARGET_DIR/linux-package/$BASENAME"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/share/applications" "$STAGE/share/mime/packages" \
  "$STAGE/share/icons/hicolor/scalable/apps" "$STAGE/share/doc/mermaidcraft"

install -m 755 "$BIN/mermaidcraft" "$BIN/mermaidcraft-cli" "$STAGE/bin/"
install -m 644 "$HERE/$APP_ID.desktop" "$STAGE/share/applications/"
install -m 644 "$HERE/$APP_ID.mime.xml" "$STAGE/share/mime/packages/$APP_ID.xml"
install -m 644 "$ROOT/assets/icon.svg" "$STAGE/share/icons/hicolor/scalable/apps/$APP_ID.svg"
install -m 644 "$ROOT/LICENSE" "$ROOT/README.md" "$STAGE/share/doc/mermaidcraft/"
for lic in "$ROOT"/assets/fonts/OFL*.txt; do
  [[ -f "$lic" ]] && install -m 644 "$lic" "$STAGE/share/doc/mermaidcraft/"
done

# PNG icons, as on macOS: rendered from assets/icon.svg. They are optional: a failure only warns.
png_icon() {
  local s="$1" out="$2"
  if command -v inkscape >/dev/null; then
    inkscape "$ROOT/assets/icon.svg" --export-type=png --export-width="$s" --export-height="$s" \
      --export-filename="$out" >/dev/null 2>&1
  elif command -v magick >/dev/null; then
    magick -background none -density 384 "$ROOT/assets/icon.svg" -resize "${s}x${s}" "$out"
  elif command -v convert >/dev/null; then
    convert -background none -density 384 "$ROOT/assets/icon.svg" -resize "${s}x${s}" "$out"
  else
    return 2
  fi
}
for s in 16 24 32 48 64 128 256 512; do
  d="$STAGE/share/icons/hicolor/${s}x${s}/apps"
  mkdir -p "$d"
  if ! png_icon "$s" "$d/$APP_ID.png"; then
    rmdir "$d" 2>/dev/null || true
    echo "warning: PNG icon ${s}x${s} not made (no working inkscape or ImageMagick); the SVG icon is used" >&2
  fi
done
find "$STAGE/share/icons/hicolor" -type d -empty -delete

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$STAGE/share/applications/$APP_ID.desktop"
fi

mkdir -p "$DIST"
# Only this build's tarball stays, so whoever picks it up by pattern gets the current version.
rm -f "$DIST"/mermaidcraft-*-linux-"$ARCH".tar.gz
tar -C "$(dirname "$STAGE")" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
"$STAGE/bin/mermaidcraft-cli" --version
echo "==> $DIST/$BASENAME.tar.gz"

refresh() {
  local tool="$1"; shift
  if ! command -v "$tool" >/dev/null; then
    echo "warning: $tool not found; run it later so the menu entry, icon and .mmd type show up" >&2
  elif ! "$tool" "$@"; then
    echo "warning: $tool failed" >&2
  fi
}
if [[ "$INSTALL" = 1 ]]; then
  echo "==> installing in $PREFIX_DIR"
  mkdir -p "$PREFIX_DIR"
  tar -xzf "$DIST/$BASENAME.tar.gz" -C "$PREFIX_DIR" --strip-components=1
  # The menu entry runs the installed binary, even before ~/.local/bin is in the session's PATH.
  desktop="$PREFIX_DIR/share/applications/$APP_ID.desktop"
  sed -i.bak "s|^Exec=mermaidcraft |Exec=$PREFIX_DIR/bin/mermaidcraft |" "$desktop" && rm -f "$desktop.bak"
  refresh update-mime-database "$PREFIX_DIR/share/mime"
  refresh update-desktop-database "$PREFIX_DIR/share/applications"
  refresh gtk-update-icon-cache -q -t "$PREFIX_DIR/share/icons/hicolor"
fi
echo "==> done"
