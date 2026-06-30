#!/bin/bash
set -e

echo "🔨 Building dyyl..."
cd "$(dirname "$0")"

# Clean and build
cargo build --release 2>&1

# Create dist directory
DIST_DIR="$(dirname "$0")/dist"
mkdir -p "$DIST_DIR"

# Copy binary
cp target/release/dyyl "$DIST_DIR/dyyl"
chmod +x "$DIST_DIR/dyyl"

# Get version
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
echo "📦 Version: $VERSION"
echo "📍 Binary: $DIST_DIR/dyyl"
echo "📏 Size: $(du -h "$DIST_DIR/dyyl" | cut -f1)"
echo ""
echo "✅ Build complete! Restart pm2 to serve new version:"
echo "   pm2 restart dyyl-server"
