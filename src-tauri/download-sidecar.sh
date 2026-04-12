#!/usr/bin/env bash
# Downloads the display-dj CLI sidecar binary from GitHub releases.
# Called by build.rs before Tauri compilation.

set -e

VERSION="v0.2.0"
REPO="synle/display-dj-cli"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
BINARIES_DIR="$(dirname "$0")/binaries"

mkdir -p "$BINARIES_DIR"

# Map Rust target triples to release asset names
case "${TAURI_ENV_TARGET_TRIPLE:-$(rustc -vV | grep host | cut -d' ' -f2)}" in
  aarch64-apple-darwin)
    ASSET="display-dj-macos-arm64"
    TARGET="display-dj-server-aarch64-apple-darwin"
    ;;
  x86_64-apple-darwin)
    ASSET="display-dj-macos-x64"
    TARGET="display-dj-server-x86_64-apple-darwin"
    ;;
  x86_64-pc-windows-msvc)
    ASSET="display-dj-windows-x64.exe"
    TARGET="display-dj-server-x86_64-pc-windows-msvc.exe"
    ;;
  aarch64-pc-windows-msvc)
    ASSET="display-dj-windows-arm64.exe"
    TARGET="display-dj-server-aarch64-pc-windows-msvc.exe"
    ;;
  x86_64-unknown-linux-gnu)
    ASSET="display-dj-linux-x64"
    TARGET="display-dj-server-x86_64-unknown-linux-gnu"
    ;;
  aarch64-unknown-linux-gnu)
    ASSET="display-dj-linux-arm64"
    TARGET="display-dj-server-aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "ERROR: Unsupported target triple: ${TAURI_ENV_TARGET_TRIPLE}" >&2
    exit 1
    ;;
esac

OUTPUT="${BINARIES_DIR}/${TARGET}"

# Skip download if binary already exists and is recent (< 1 day old)
if [ -f "$OUTPUT" ] && [ "$(find "$OUTPUT" -mtime -1 2>/dev/null)" ]; then
  echo "Sidecar binary already exists and is recent: $OUTPUT"
  exit 0
fi

echo "Downloading display-dj ${VERSION} for ${TARGET}..."
curl -fSL "${BASE_URL}/${ASSET}" -o "$OUTPUT"
chmod +x "$OUTPUT"
echo "Downloaded: $OUTPUT ($(wc -c < "$OUTPUT" | tr -d ' ') bytes)"
