#!/usr/bin/env bash
set -euo pipefail

# Standardized icon pipeline:
# 1) Build a single 1024x1024 master icon from source art.
# 2) Use `cargo tauri icon` to generate all platform icon outputs.
# 3) Leave tray icon unchanged by default.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SOURCE_ART="${1:-web/logo.png}"
REGENERATE_TRAY_ICON="${REGENERATE_TRAY_ICON:-0}"
TRAY_ART="${TRAY_SOURCE_ART:-web/logo.png}"
MASTER_ICON="$(mktemp /tmp/flavortime-app-icon-master.XXXXXX.png)"
STANDARD_OUT_DIR="$(mktemp -d /tmp/flavortime-tauri-icons.XXXXXX)"

cleanup() {
  rm -f "$MASTER_ICON"
  rm -rf "$STANDARD_OUT_DIR"
}
trap cleanup EXIT

if [[ ! -f "$SOURCE_ART" ]]; then
  echo "Source art not found: $SOURCE_ART" >&2
  exit 1
fi

if ! command -v magick >/dev/null 2>&1; then
  echo "ImageMagick (magick) is required." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required." >&2
  exit 1
fi

CANVAS=1024
magick "$SOURCE_ART" -auto-orient -resize "${CANVAS}x${CANVAS}" \
  -gravity center -background none -extent "${CANVAS}x${CANVAS}" \
  "$MASTER_ICON"

# Standard Tauri icon generation (writes all platform assets to temp first).
cargo tauri icon "$MASTER_ICON" -o "$STANDARD_OUT_DIR"

# Copy only the desktop and Windows tile assets we use in this project.
cp "$STANDARD_OUT_DIR/32x32.png" icons/32x32.png
cp "$STANDARD_OUT_DIR/128x128.png" icons/128x128.png
cp "$STANDARD_OUT_DIR/128x128@2x.png" icons/128x128@2x.png
cp "$STANDARD_OUT_DIR/icon.png" icons/icon.png
cp "$STANDARD_OUT_DIR/icon.icns" icons/icon.icns
cp "$STANDARD_OUT_DIR/icon.ico" icons/icon.ico

if [[ "$REGENERATE_TRAY_ICON" == "1" ]]; then
  if [[ ! -f "$TRAY_ART" ]]; then
    echo "Tray source art not found: $TRAY_ART" >&2
    exit 1
  fi

  # macOS template tray icon: must be BLACK on transparent (macOS auto-tints to match menu bar).
  # Target: 44px tall (@2x retina), fit proportionally.
  # Build tray icon from alpha mask for a stable monochrome template result.
  TRAY_ALPHA="$(mktemp /tmp/flavortime-tray-alpha.XXXXXX.png)"
  magick "$TRAY_ART" -alpha extract -trim +repage -resize x44 -gravity center -background black -extent 44x44 "$TRAY_ALPHA"
  magick -size 44x44 xc:black "$TRAY_ALPHA" -alpha off -compose copyopacity -composite icons/trayTemplate.png
  rm -f "$TRAY_ALPHA"
fi

echo "Icon generation complete."
md5 icons/icon.png icons/icon.icns icons/icon.ico icons/trayTemplate.png
