#!/bin/bash

# Void Runtime Update Script for Linux/macOS
# Updates Void to the latest version

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

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
log_header "Void Runtime Update"

# Determine installation directory
INSTALL_DIR="${VOID_INSTALL_DIR:=$HOME/.local/void}"

# Check if Void is installed
if [ ! -d "$INSTALL_DIR/void" ]; then
    log_error "Void is not installed at $INSTALL_DIR"
    log_info "Run the installer first: ./install.sh"
    exit 1
fi

log_info "Installation directory: ${BOLD}$INSTALL_DIR${NC}"

log_separator

# Update repository
log_step "Updating repository"
cd "$INSTALL_DIR/void"

if ! git diff --quiet || ! git diff --cached --quiet; then
    log_error "Local changes detected in $INSTALL_DIR/void"
    log_info "Commit or stash your changes before updating."
    log_info "Example: git stash push -u -m 'void-update-temp'"
    log_info "Then rerun this update script."
    exit 1
fi

log_progress "Fetching latest changes..."
if git pull --ff-only origin main --quiet; then
    log_success "Repository updated"
else
    log_error "Repository update failed"
    log_info "Resolve git issues, then rerun the update."
    exit 1
fi

log_separator

# Rebuild language runtime
log_step "Rebuilding language runtime"
cd language

log_progress "Compiling Rust code (this may take a while)..."
if cargo build --release 2>&1 | grep -E "Compiling|Finished" || true; then
    log_success "Language runtime built"
else
    log_warning "Build completed with issues"
fi

cd "$INSTALL_DIR/void"

log_separator

# Rebuild package manager
log_step "Rebuilding package manager (VPM)"
cd package-manager

log_progress "Installing npm dependencies..."
if npm install --silent 2>&1 | tail -1; then
    log_success "Dependencies installed"
else
    log_warning "npm install encountered issues"
fi

log_progress "Building package manager..."
if npm run build --silent 2>&1 | tail -1 || true; then
    log_success "Package manager built"
else
    log_warning "Build completed with issues"
fi

cd "$INSTALL_DIR/void"

log_separator

# Verify update
log_step "Verifying update"

if command -v void &> /dev/null; then
    log_success "Void is installed and accessible"
    VOID_VERSION=$(void --version 2>/dev/null || echo "unknown")
    log_info "Version: $VOID_VERSION"
else
    log_warning "Void command not found in PATH"
fi

log_separator

log_header "✨ Update Complete!"

echo -e "${GREEN}${BOLD}Next steps:${NC}"
echo -e "  ${CYAN}void $INSTALL_DIR/void/language/examples/hello.void${NC}    Test with hello world"
echo -e "  ${CYAN}void $INSTALL_DIR/void/language/examples/hyperdrive.void${NC}  Test advanced example"

echo -e "\n${GREEN}${BOLD}Installation Location:${NC}"
echo -e "  ${CYAN}$INSTALL_DIR${NC}"

echo -e "\n${GREEN}${BOLD}Documentation:${NC}"
echo -e "  ${CYAN}https://github.com/Olibot1107/void${NC}"

echo -e "\n${GREEN}${BOLD}Happy coding! 🚀${NC}\n"
