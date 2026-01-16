#!/bin/bash
#
# Install Script for cosmic-runkat
# =================================
#
# Compiles the Rust application and installs it as a COSMIC desktop tray app.
#
# USAGE:
#   ./install.sh           Install for current user (~/.local)
#   sudo ./install.sh      Install system-wide (/usr/local)
#
# WHAT IT DOES:
#   1. Builds the release binary with cargo
#   2. Installs the binary to bin directory
#   3. Installs the desktop file (for settings app in Applications)
#   4. Sets up autostart for the tray
#
# TO UNINSTALL:
#   ./install.sh --uninstall
#   sudo ./install.sh --uninstall
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_NAME="cosmic-runkat"
APP_ID="io.github.cosmic-runkat"

# Determine install prefix based on whether running as root
if [[ $EUID -eq 0 ]]; then
    PREFIX="/usr/local"
    echo "Installing system-wide to $PREFIX..."
else
    PREFIX="$HOME/.local"
    echo "Installing for current user to $PREFIX..."
fi

BIN_DIR="$PREFIX/bin"
SHARE_DIR="$PREFIX/share"
APPLICATIONS_DIR="$SHARE_DIR/applications"
AUTOSTART_DIR="$HOME/.config/autostart"

# Handle uninstall
if [[ "${1:-}" == "--uninstall" ]] || [[ "${1:-}" == "-u" ]]; then
    echo "Uninstalling $APP_NAME..."

    # Stop running instances
    pkill -f "$APP_NAME" 2>/dev/null || true

    rm -f "$BIN_DIR/$APP_NAME"
    rm -f "$APPLICATIONS_DIR/$APP_ID.desktop"
    rm -f "$AUTOSTART_DIR/$APP_ID-tray.desktop"

    # Update desktop database
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database "$APPLICATIONS_DIR" 2>/dev/null || true
    fi

    echo "Uninstall complete."
    exit 0
fi

# Build release binary
echo ""
echo "=== Building release binary ==="
cd "$SCRIPT_DIR"
cargo build --release

# Stop any running instances before upgrading
echo ""
echo "=== Stopping running instances ==="
if pgrep -f "$APP_NAME" > /dev/null 2>&1; then
    echo "Stopping running processes..."
    pkill -f "$APP_NAME" 2>/dev/null || true
    sleep 1
fi

# Create directories
echo ""
echo "=== Installing files ==="
mkdir -p "$BIN_DIR"
mkdir -p "$APPLICATIONS_DIR"
mkdir -p "$AUTOSTART_DIR"

# Install binary (remove old first to avoid permission issues)
echo "Installing binary..."
rm -f "$BIN_DIR/$APP_NAME"
cp "$SCRIPT_DIR/target/release/$APP_NAME" "$BIN_DIR/"
chmod +x "$BIN_DIR/$APP_NAME"

# Install desktop file for settings app
echo "Installing desktop entry..."
cat > "$APPLICATIONS_DIR/$APP_ID.desktop" << EOF
[Desktop Entry]
Name=RunKat Settings
Comment=Configure the running cat CPU indicator
GenericName=CPU Monitor Settings
Exec=$BIN_DIR/$APP_NAME --settings
Icon=preferences-system
Terminal=false
Type=Application
Categories=Settings;System;
Keywords=cpu;monitor;cat;runkat;settings;
StartupNotify=true
EOF

# Install autostart entry for tray
echo "Installing autostart entry..."
cat > "$AUTOSTART_DIR/$APP_ID-tray.desktop" << EOF
[Desktop Entry]
Name=RunKat Tray
Comment=Running cat CPU indicator
Exec=$BIN_DIR/$APP_NAME --tray
Icon=utilities-system-monitor
Terminal=false
Type=Application
Categories=System;Monitor;
X-GNOME-Autostart-enabled=true
EOF

# Update desktop database
if command -v update-desktop-database &> /dev/null; then
    echo "Updating desktop database..."
    update-desktop-database "$APPLICATIONS_DIR" 2>/dev/null || true
fi

echo ""
echo "=== Installation complete ==="
echo ""
echo "The tray app will start automatically on next login."
echo "To start it now, run: $APP_NAME --tray"
echo ""
echo "Settings are available in your Applications menu as 'RunKat Settings'."
echo "Or from the tray menu by clicking on the cat icon."
echo ""
echo "To uninstall, run: $0 --uninstall"
