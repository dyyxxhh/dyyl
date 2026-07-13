#!/usr/bin/env bash
# Pack a plugin, compute SHA256, and generate manifest.json.
#
# Usage: ./scripts/publish-plugin.sh <plugin_name> <version> <lib_path>
#
# Outputs to dist/plugins/<name>/manifest.json and
# dist/plugins/<name>/<version>/<platform>/<filename>.

set -euo pipefail

if [ $# -ne 3 ]; then
  echo "Usage: $0 <plugin_name> <version> <lib_path>" >&2
  exit 1
fi

NAME="$1"
VERSION="$2"
LIB_PATH="$3"

if [ ! -f "$LIB_PATH" ]; then
  echo "error: library not found: $LIB_PATH" >&2
  exit 1
fi

# Detect platform.
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
PLATFORM="${OS}-${ARCH}"

# Determine filename.
FILENAME="$(basename "$LIB_PATH")"

# Output directory.
OUT_DIR="dist/plugins/${NAME}/${VERSION}/${PLATFORM}"
mkdir -p "$OUT_DIR"

# Copy library.
cp "$LIB_PATH" "$OUT_DIR/$FILENAME"

# Compute SHA256.
SHA256="$(sha256sum "$OUT_DIR/$FILENAME" | cut -d' ' -f1)"

# Generate manifest.json.
MANIFEST="dist/plugins/${NAME}/manifest.json"
mkdir -p "$(dirname "$MANIFEST")"

# Build URL (assumes l.dyyapp.com or localhost for dev).
BASE_URL="${PLUGIN_BASE_URL:-https://l.dyyapp.com}"
URL="${BASE_URL}/plugins/${NAME}/${VERSION}/${PLATFORM}/${FILENAME}"

cat > "$MANIFEST" <<EOF
{
  "name": "${NAME}",
  "version": "${VERSION}",
  "abi_version": 1,
  "dyyl_min": "0.2.0",
  "panic_mode": "abort",
  "commands": [],
  "platforms": [
    {"platform": "${PLATFORM}", "url": "${URL}", "sha256": "${SHA256}"}
  ]
}
EOF

echo "Published ${NAME} ${VERSION} to ${OUT_DIR}"
echo "Manifest: ${MANIFEST}"
echo "SHA256: ${SHA256}"
