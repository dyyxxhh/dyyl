const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = 8951;
const DIST_DIR = path.join(__dirname, 'dist');
const BINARY_PATH = path.join(DIST_DIR, 'dyyl');

const INSTALL_SCRIPT = `#!/bin/bash
set -e

echo "🚀 Installing dyyl..."

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
    echo "❌ Unsupported architecture: $ARCH (only x86_64 supported)"
    exit 1
fi

# Create install directory
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# Download binary
echo "📥 Downloading dyyl..."
curl -L -o "$INSTALL_DIR/dyyl" "https://l.dyyapp.com/download"
chmod +x "$INSTALL_DIR/dyyl"

# Add to PATH if needed
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "" >> ~/.bashrc
    echo "# Add dyyl to PATH" >> ~/.bashrc
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
    export PATH="$INSTALL_DIR:$PATH"
fi

echo "✅ dyyl installed successfully!"
echo "   Binary: $INSTALL_DIR/dyyl"
echo ""
echo "Run 'dyyl --help' to get started."
`;

const server = http.createServer((req, res) => {
    console.log(`${new Date().toISOString()} ${req.method} ${req.url}`);

    if (req.url === '/' || req.url === '/install' || req.url === '/install/') {
        res.writeHead(200, {
            'Content-Type': 'application/x-sh',
            'Content-Disposition': 'attachment; filename="install.sh"'
        });
        res.end(INSTALL_SCRIPT);
        return;
    }

    // Binary download
    if (req.url === '/download' || req.url === '/download/') {
        if (!fs.existsSync(BINARY_PATH)) {
            res.writeHead(404);
            res.end('Binary not found. Run build.sh first.');
            return;
        }
        const stat = fs.statSync(BINARY_PATH);
        res.writeHead(200, {
            'Content-Type': 'application/octet-stream',
            'Content-Length': stat.size,
            'Content-Disposition': 'attachment; filename="dyyl"'
        });
        fs.createReadStream(BINARY_PATH).pipe(res);
        return;
    }

    // Block everything else
    res.writeHead(403);
    res.end('Access denied');
});

server.listen(PORT, '0.0.0.0', () => {
    console.log(`dyyl install server running on http://0.0.0.0:${PORT}`);
    console.log(`Install with: curl -L http://192.168.3.66:${PORT}/install | bash`);
});
