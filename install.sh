#!/bin/bash

# ============================================================================
# corsair-top Install Script
# Build, install, and configure corsair-top for monitoring Corsair AX1600i PSU
# ============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
error()   { echo -e "${RED}[ERROR]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_NAME="corsair-top"
INSTALL_PATH="/usr/local/bin/${BINARY_NAME}"
UDEV_RULE_PATH="/etc/udev/rules.d/99-corsair-psu.rules"
BASHRC="$HOME/.bashrc"
ALIAS_LINE="alias corsair-top='/usr/local/bin/corsair-top-launch'"

echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║       corsair-top Install Script             ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════╝${NC}"
echo ""

# ============================================================================
# Step 1: Check for required dependencies
# ============================================================================

info "Checking required dependencies..."

# Check for cargo/rustc
if command -v cargo &>/dev/null && command -v rustc &>/dev/null; then
    success "Rust toolchain found ($(rustc --version))"
else
    error "Rust toolchain (cargo/rustc) not found."
    echo ""
    echo "  Install Rust via rustup with:"
    echo ""
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "  Then restart your shell and re-run this script."
    exit 1
fi

# Check for libusb-1.0-0-dev
if dpkg -s libusb-1.0-0-dev &>/dev/null; then
    success "libusb-1.0-0-dev is installed"
else
    warn "libusb-1.0-0-dev not found. Installing..."
    sudo apt-get update -qq && sudo apt-get install -y libusb-1.0-0-dev
    if [ $? -eq 0 ]; then
        success "libusb-1.0-0-dev installed successfully"
    else
        error "Failed to install libusb-1.0-0-dev"
        exit 1
    fi
fi

# Check for pkg-config
if command -v pkg-config &>/dev/null; then
    success "pkg-config is installed"
else
    warn "pkg-config not found. Installing..."
    sudo apt-get update -qq && sudo apt-get install -y pkg-config
    if [ $? -eq 0 ]; then
        success "pkg-config installed successfully"
    else
        error "Failed to install pkg-config"
        exit 1
    fi
fi

echo ""

# ============================================================================
# Step 2: Build the project in release mode
# ============================================================================

info "Building corsair-top in release mode..."
cd "$SCRIPT_DIR"

if cargo build --release; then
    success "Build completed successfully"
else
    error "Build failed. Check the output above for details."
    exit 1
fi

echo ""

# ============================================================================
# Step 3: Install the binary to /usr/local/bin
# ============================================================================

info "Installing binary to ${INSTALL_PATH}..."

if [ -f "$SCRIPT_DIR/target/release/${BINARY_NAME}" ]; then
    sudo cp "$SCRIPT_DIR/target/release/${BINARY_NAME}" "$INSTALL_PATH"
    sudo chmod +x "$INSTALL_PATH"
    success "Binary installed to ${INSTALL_PATH}"
else
    error "Built binary not found at target/release/${BINARY_NAME}"
    exit 1
fi

# Install launch script
LAUNCHER_PATH="/usr/local/bin/${BINARY_NAME}-launch"
sudo cp "$SCRIPT_DIR/corsair-top.sh" "$LAUNCHER_PATH"
sudo chmod +x "$LAUNCHER_PATH"
success "Launch script installed to ${LAUNCHER_PATH}"

echo ""

# ============================================================================
# Step 4: Install desktop launcher and icon (optional)
# ============================================================================

read -rp "$(echo -e "${BLUE}[?]${NC} Install desktop launcher with icon? [y/N]: ")" INSTALL_DESKTOP

if [[ "$INSTALL_DESKTOP" =~ ^[Yy]$ ]]; then
    # Install icon
    ICON_DIR="/usr/share/icons/hicolor/256x256/apps"
    sudo mkdir -p "$ICON_DIR"
    sudo cp "$SCRIPT_DIR/logo/corsairtop.png" "$ICON_DIR/corsair-top.png"
    success "Icon installed to ${ICON_DIR}/corsair-top.png"

    # Install .desktop file system-wide
    sudo cp "$SCRIPT_DIR/corsair-top.desktop" /usr/share/applications/corsair-top.desktop
    success "Desktop entry installed to /usr/share/applications/corsair-top.desktop"

    # Copy .desktop file to user's Desktop for easy access
    DESKTOP_DIR="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")"
    if [ -d "$DESKTOP_DIR" ]; then
        cp "$SCRIPT_DIR/corsair-top.desktop" "$DESKTOP_DIR/corsair-top.desktop"
        chmod +x "$DESKTOP_DIR/corsair-top.desktop"
        success "Launcher copied to ${DESKTOP_DIR}/corsair-top.desktop"
    else
        warn "Desktop directory not found. You can manually copy corsair-top.desktop to your desktop."
    fi

    # Update icon cache
    if command -v gtk-update-icon-cache &>/dev/null; then
        sudo gtk-update-icon-cache -f /usr/share/icons/hicolor/ 2>/dev/null
    fi
else
    info "Skipping desktop launcher installation."
fi

echo ""

# ============================================================================
# Step 5: Add shell alias to ~/.bashrc
# ============================================================================

info "Configuring shell alias..."

if grep -qF "$ALIAS_LINE" "$BASHRC" 2>/dev/null; then
    success "Alias already present in ${BASHRC}"
else
    echo "" >> "$BASHRC"
    echo "# corsair-top - Corsair PSU monitor" >> "$BASHRC"
    echo "$ALIAS_LINE" >> "$BASHRC"
    success "Alias added to ${BASHRC}"
fi

echo ""

# ============================================================================
# Step 6: Create udev rule (optional)
# ============================================================================

info "Setting up udev rule for USB access without sudo..."

UDEV_RULE='SUBSYSTEM=="usb", ATTR{idVendor}=="1b1c", ATTR{idProduct}=="1c11", MODE="0666", GROUP="plugdev"'

if [ -f "$UDEV_RULE_PATH" ] && grep -qF "$UDEV_RULE" "$UDEV_RULE_PATH" 2>/dev/null; then
    success "udev rule already in place at ${UDEV_RULE_PATH}"
else
    echo "$UDEV_RULE" | sudo tee "$UDEV_RULE_PATH" > /dev/null
    sudo udevadm control --reload-rules && sudo udevadm trigger
    success "udev rule created at ${UDEV_RULE_PATH}"
    info "Users in the 'plugdev' group can now access the PSU without sudo."
    info "Add your user to plugdev with: sudo usermod -aG plugdev \$USER"
fi

echo ""

# ============================================================================
# Step 7: Success message and usage instructions
# ============================================================================

echo -e "${GREEN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║       Installation Complete!                 ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}Usage:${NC}"
echo "  corsair-top          Launch with splash logo (via alias)"
echo "  sudo corsair-top     Run directly (no splash)"
echo ""
echo -e "${YELLOW}NOTE:${NC} To use the alias in your current session, run:"
echo ""
echo "  source ~/.bashrc"
echo ""
echo "  Or simply open a new terminal."
echo ""
echo -e "${BLUE}If you set up the udev rule and added your user to the 'plugdev' group,${NC}"
echo -e "${BLUE}you can run corsair-top without sudo after logging out and back in.${NC}"
echo ""
