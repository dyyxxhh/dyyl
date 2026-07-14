#!/usr/bin/env bash
# Pack a plugin, compute SHA256, and generate manifest.json.
#
# Usage:
#   ./scripts/publish-plugin.sh <source_dir> [--target <rustc-target>]
#   ./scripts/publish-plugin.sh <plugin_name> <version> <lib_path>
#
# Outputs to dist/plugins/<name>/manifest.json and
# dist/plugins/<name>/<version>/<platform>/<filename>.

set -euo pipefail

# ── Mode detection ─────────────────────────────────────────────

if [ $# -ge 1 ] && [ -d "$1" ]; then
  # New mode: source directory
  SOURCE_DIR="$1"
  shift

  # Parse optional --target
  TARGET=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --target) TARGET="$2"; shift 2 ;;
      *) echo "unknown option: $1" >&2; exit 1 ;;
    esac
  done

  # Read plugin.toml.in fields using Python
  TOML_FILE="${SOURCE_DIR}/plugin.toml.in"
  if [ ! -f "$TOML_FILE" ]; then
    echo "error: ${TOML_FILE} not found" >&2
    exit 1
  fi

  # Extract fields using Python (handles TOML properly)
  PYTHON_PARSE_FAILED=0
  eval "$(python3 -c "
import sys
try:
    import tomllib
except ImportError:
    import tomli as tomllib
import json

with open('${TOML_FILE}', 'rb') as f:
    data = tomllib.load(f)

print(f'NAME={json.dumps(data[\"name\"])}')
print(f'VERSION={json.dumps(data[\"version\"])}')
print(f'ABI_VERSION={data[\"abi_version\"]}')
print(f'DYYL_MIN={json.dumps(data[\"dyyl_min\"])}')
print(f'PANIC_MODE={json.dumps(data[\"panic_mode\"])}')
print(f'COMMANDS_JSON={json.dumps(json.dumps(data.get(\"commands\", [])))}')
print(f'CREDENTIALS_JSON={json.dumps(json.dumps(data.get(\"credentials\", None)))}')
" 2>/dev/null || echo "PYTHON_PARSE_FAILED=1")"

  if [ "${PYTHON_PARSE_FAILED:-0}" = "1" ]; then
    # Fallback: parse with grep/sed for basic fields
    NAME="$(grep '^name' "$TOML_FILE" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
    VERSION="$(grep '^version' "$TOML_FILE" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
    ABI_VERSION="$(grep '^abi_version' "$TOML_FILE" | head -1 | sed 's/.*= *//')"
    DYYL_MIN="$(grep '^dyyl_min' "$TOML_FILE" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
    PANIC_MODE="$(grep '^panic_mode' "$TOML_FILE" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
    # Use command_list.json directly for commands
    COMMANDS_JSON="$(cat "${SOURCE_DIR}/command_list.json" 2>/dev/null || echo '[]')"
    # No credentials in fallback mode
    CREDENTIALS_JSON="null"
  fi

  # Build the plugin
  echo "[publish] Building plugin in ${SOURCE_DIR}..."
  BUILD_ARGS=""
  if [ -n "$TARGET" ]; then
    BUILD_ARGS="--target ${TARGET}"
  fi
  (cd "$SOURCE_DIR" && cargo build --release $BUILD_ARGS)

  # Find the built library
  if [ -n "$TARGET" ]; then
    RELEASE_DIR="${SOURCE_DIR}/target/${TARGET}/release"
  else
    RELEASE_DIR="${SOURCE_DIR}/target/release"
  fi

  if [ -f "${RELEASE_DIR}/lib${NAME}.so" ]; then
    LIB_PATH="${RELEASE_DIR}/lib${NAME}.so"
    FILENAME="lib${NAME}.so"
  elif [ -f "${RELEASE_DIR}/lib${NAME}.dylib" ]; then
    LIB_PATH="${RELEASE_DIR}/lib${NAME}.dylib"
    FILENAME="lib${NAME}.dylib"
  elif [ -f "${RELEASE_DIR}/${NAME}.dll" ]; then
    LIB_PATH="${RELEASE_DIR}/${NAME}.dll"
    FILENAME="${NAME}.dll"
  else
    echo "error: no built library found in ${RELEASE_DIR}/" >&2
    exit 1
  fi

  # Detect platform
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  ARCH="$(uname -m)"
  PLATFORM="${OS}-${ARCH}"

  # Output directory
  OUT_DIR="dist/plugins/${NAME}/${VERSION}/${PLATFORM}"
  mkdir -p "$OUT_DIR"

  # Copy library
  cp "$LIB_PATH" "$OUT_DIR/$FILENAME"

  # Compute SHA256
  SHA256="$(sha256sum "$OUT_DIR/$FILENAME" | cut -d' ' -f1)"

  # Generate manifest.json
  MANIFEST="dist/plugins/${NAME}/manifest.json"
  mkdir -p "$(dirname "$MANIFEST")"

  # Build URL
  BASE_URL="${DYRL_DIST_HOST:-http://localhost:8951}"
  URL="${BASE_URL}/plugins/${NAME}/${VERSION}/${PLATFORM}/${FILENAME}"

  # Generate manifest using Python (for proper JSON with commands + credentials).
  # Pass data via env vars to avoid quoting issues with embedded JSON.
  _PP_NAME="$NAME" _PP_VERSION="$VERSION" _PP_ABI="$ABI_VERSION" \
  _PP_DYYL_MIN="$DYYL_MIN" _PP_PANIC="$PANIC_MODE" \
  _PP_COMMANDS="$COMMANDS_JSON" _PP_CREDENTIALS="$CREDENTIALS_JSON" \
  _PP_PLATFORM="$PLATFORM" _PP_URL="$URL" _PP_SHA256="$SHA256" \
  python3 -c "
import json, os, sys

manifest = {
    'name': os.environ['_PP_NAME'],
    'version': os.environ['_PP_VERSION'],
    'abi_version': int(os.environ['_PP_ABI']),
    'dyyl_min': os.environ['_PP_DYYL_MIN'],
    'panic_mode': os.environ['_PP_PANIC'],
    'commands': json.loads(os.environ['_PP_COMMANDS']),
    'platforms': [
        {'platform': os.environ['_PP_PLATFORM'], 'url': os.environ['_PP_URL'], 'sha256': os.environ['_PP_SHA256']}
    ]
}

creds = os.environ['_PP_CREDENTIALS']
if creds != 'null':
    manifest['credentials'] = json.loads(creds)

json.dump(manifest, sys.stdout, indent=2, ensure_ascii=False)
" > "$MANIFEST" 2>/dev/null || {
    # Fallback: simple manifest without commands/credentials
    cat > "$MANIFEST" <<EOF
{
  "name": "${NAME}",
  "version": "${VERSION}",
  "abi_version": ${ABI_VERSION},
  "dyyl_min": "${DYYL_MIN}",
  "panic_mode": "${PANIC_MODE}",
  "commands": [],
  "platforms": [
    {"platform": "${PLATFORM}", "url": "${URL}", "sha256": "${SHA256}"}
  ]
}
EOF
  }

  echo "Published ${NAME} ${VERSION} to ${OUT_DIR}"
  echo "Manifest: ${MANIFEST}"
  echo "SHA256: ${SHA256}"

elif [ $# -eq 3 ]; then
  # Old mode: name, version, lib_path
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
  BASE_URL="${DYRL_DIST_HOST:-https://l.dyyapp.com}"
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

else
  echo "Usage: $0 <source_dir> [--target <target>]" >&2
  echo "       $0 <plugin_name> <version> <lib_path>" >&2
  exit 1
fi
