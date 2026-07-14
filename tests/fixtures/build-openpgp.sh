#!/usr/bin/env bash
# Build the OpenPGP plugin and copy the cdylib to the directory specified by $1.
# Usage: ./tests/fixtures/build-openpgp.sh <output_dir>
set -euo pipefail

OUTPUT_DIR="${1:?usage: build-openpgp.sh <output_dir>}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_DIR="$SCRIPT_DIR/../../plugins/openpgp"

echo "[build-openpgp] Building plugin in $PLUGIN_DIR..."
(cd "$PLUGIN_DIR" && cargo build --release)

# Find the built library (platform-dependent extension)
if [[ -f "$PLUGIN_DIR/target/release/libopenpgp.so" ]]; then
    cp "$PLUGIN_DIR/target/release/libopenpgp.so" "$OUTPUT_DIR/"
elif [[ -f "$PLUGIN_DIR/target/release/libopenpgp.dylib" ]]; then
    cp "$PLUGIN_DIR/target/release/libopenpgp.dylib" "$OUTPUT_DIR/"
elif [[ -f "$PLUGIN_DIR/target/release/openpgp.dll" ]]; then
    cp "$PLUGIN_DIR/target/release/openpgp.dll" "$OUTPUT_DIR/"
else
    echo "[build-openpgp] ERROR: no built library found in $PLUGIN_DIR/target/release/" >&2
    exit 1
fi

echo "[build-openpgp] Plugin copied to $OUTPUT_DIR"
