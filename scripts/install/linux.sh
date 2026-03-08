#!/bin/bash

# Void Runtime Installer for Linux/macOS
# This script automatically clones and sets up the Void scripting runtime

set -e  # Exit on error

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Logging functions
log_header() {
    echo -e "\n${CYAN}${BOLD}════════════════════════════════════════${NC}"
    echo -e "${CYAN}${BOLD}   $1${NC}"
    echo -e "${CYAN}${BOLD}════════════════════════════════════════${NC}\n"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} ${RED}$1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} ${YELLOW}$1${NC}"
}

log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

log_step() {
    echo -e "\n${BOLD}${CYAN}→${NC} ${BOLD}$1${NC}"
}

log_progress() {
    echo -e "${BLUE}  ⟳${NC} $1"
}

log_separator() {
    echo -e "${CYAN}─────────────────────────────────────────${NC}"
}

# Main script
log_header "Void Scripting Runtime Installer"

# Check if git is installed
log_step "Checking prerequisites"
log_progress "Checking Git..."
if ! command -v git &> /dev/null; then
    log_error "Git is not installed"
    log_info "Please install Git: https://git-scm.com/download"
    exit 1
fi
log_success "Git found"

# Check if Rust is installed
log_progress "Checking Rust..."
if ! command -v rustc &> /dev/null; then
    log_warning "Rust is not installed"
    echo -e "${CYAN}Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
    log_success "Rust installed successfully"
else
    log_success "Rust found"
fi

# Check if Node.js is installed
log_progress "Checking Node.js..."
if ! command -v node &> /dev/null; then
    log_warning "Node.js is not installed"
    read -p "$(echo -e ${CYAN})Continue anyway? (y/n)${NC} " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
else
    log_success "Node.js found"
fi

log_separator

# Set installation directory
INSTALL_DIR="${VOID_INSTALL_DIR:=$HOME/.local/void}"
log_step "Setup"
log_info "Installation directory: ${BOLD}$INSTALL_DIR${NC}"

# Create installation directory
mkdir -p "$INSTALL_DIR"
log_success "Installation directory ready"

log_separator

# Clone the repository
log_step "Cloning repository"
if [ -d "$INSTALL_DIR/void" ]; then
    log_progress "Void directory exists, updating..."
    cd "$INSTALL_DIR/void"
    git pull origin main --quiet
    log_success "Repository updated"
else
    log_progress "Cloning from GitHub..."
    git clone https://github.com/Olibot1107/void.git "$INSTALL_DIR/void" --quiet
    log_success "Repository cloned"
fi

cd "$INSTALL_DIR/void"

log_separator

# Build the language runtime
log_step "Building language runtime"
cd language

if [ -f "Cargo.toml" ]; then
    log_progress "Compiling Rust code (this may take a while)..."
    cargo build --release 2>&1 | grep -E "Compiling|Finished" || true
    log_success "Language runtime built"
else
    log_warning "Cargo.toml not found in language directory"
fi

cd "$INSTALL_DIR/void"

log_separator

# Build the package manager
log_step "Building package manager (VPM)"
cd package-manager

if [ -f "package.json" ]; then
    log_progress "Installing npm dependencies..."
    npm install --silent 2>&1 | tail -1
    log_success "Dependencies installed"
    
    log_progress "Building package manager..."
    npm run build --silent 2>&1 | tail -1 || true
    log_success "Package manager built"
else
    log_warning "package.json not found in package-manager directory"
fi

cd "$INSTALL_DIR/void"

log_separator

# Create symlinks for easy access
log_step "Setting up command shortcuts"

mkdir -p "$HOME/.local/bin"

# Create void executable symlink
if [ -f "language/target/release/void" ]; then
    ln -sf "$INSTALL_DIR/void/language/target/release/void" "$HOME/.local/bin/void"
    log_success "'void' command linked"
else
    log_warning "Void executable not found"
fi

# Create vpm executable symlink
VPM_BIN=$(find package-manager -name "vpm" -type f 2>/dev/null | head -1)
if [ ! -z "$VPM_BIN" ]; then
    ln -sf "$INSTALL_DIR/void/$VPM_BIN" "$HOME/.local/bin/vpm"
    log_success "'vpm' command linked"
fi

# Expose examples at a stable path for docs/quick-start commands.
if [ -d "$INSTALL_DIR/void/language/examples" ]; then
    ln -sfn "$INSTALL_DIR/void/language/examples" "$INSTALL_DIR/examples"
    log_success "Examples linked at $INSTALL_DIR/examples"
fi

log_separator

# Update PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    log_warning "~/.local/bin is not in your PATH"
    log_info "Add this line to ${BOLD}~/.bashrc${NC}, ${BOLD}~/.zshrc${NC}, or equivalent:"
    echo -e "${GREEN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
else
    log_success "PATH is properly configured"
fi

log_separator

# Run a test
log_step "Verifying installation"
if command -v void &> /dev/null; then
    log_success "Void is installed and accessible!"
    VOID_VERSION=$(void --version 2>/dev/null || echo "unknown")
    log_info "Version: $VOID_VERSION"
else
    log_warning "Void command not found in PATH"
    log_info "Try restarting your terminal or updating your PATH"
fi

log_separator

log_header "✨ Installation Complete!"

echo -e "${GREEN}${BOLD}Quick Start:${NC}"
echo -e "  ${CYAN}void $INSTALL_DIR/void/language/examples/hello.void${NC}    Run hello world"
echo -e "  ${CYAN}void $INSTALL_DIR/void/language/examples/main.void${NC}     Run main example"

echo -e "\n${GREEN}${BOLD}Installation Location:${NC}"
echo -e "  ${CYAN}$INSTALL_DIR${NC}"

echo -e "\n${GREEN}${BOLD}Documentation:${NC}"
echo -e "  ${CYAN}https://github.com/Olibot1107/void${NC}"

echo -e "\n${GREEN}${BOLD}Enjoy coding with Void! 🚀${NC}\n"
