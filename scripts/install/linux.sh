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

confirm() {
    local prompt="$1"
    local reply=""
    local input=""

    # When installer is piped (curl | bash), stdin is the script itself.
    # Always read confirmations from the terminal instead.
    if [ -r /dev/tty ]; then
        input="/dev/tty"
    elif [ -t 0 ]; then
        input="/dev/stdin"
    else
        return 2
    fi

    while true; do
        if ! read -r -p "$prompt [Y/n]: " reply < "$input"; then
            return 2
        fi
        reply="${reply:-Y}"
        case "$reply" in
            [Yy]* ) return 0 ;;
            [Nn]* ) return 1 ;;
            * ) echo "Please answer Y or N." ;;
        esac
    done
}

run_privileged() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo &> /dev/null; then
        sudo "$@"
    else
        return 1
    fi
}

load_rust_env() {
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1090
        source "$HOME/.cargo/env"
    fi
    if [ -x "$HOME/.cargo/bin/rustc" ] && [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    fi
}

install_git_auto() {
    local os_name
    os_name="$(uname -s)"

    case "$os_name" in
        Darwin)
            if command -v brew &> /dev/null; then
                brew install git
                return $?
            fi
            xcode-select --install >/dev/null 2>&1 || true
            return 1
            ;;
        Linux)
            if command -v apt-get &> /dev/null; then
                run_privileged apt-get update
                run_privileged apt-get install -y git
            elif command -v dnf &> /dev/null; then
                run_privileged dnf install -y git
            elif command -v yum &> /dev/null; then
                run_privileged yum install -y git
            elif command -v pacman &> /dev/null; then
                run_privileged pacman -Sy --noconfirm git
            elif command -v zypper &> /dev/null; then
                run_privileged zypper --non-interactive install git
            elif command -v apk &> /dev/null; then
                run_privileged apk add --no-cache git
            else
                return 1
            fi
            ;;
        *)
            return 1
            ;;
    esac

    command -v git &> /dev/null
}

install_c_toolchain_auto() {
    local os_name
    os_name="$(uname -s)"

    case "$os_name" in
        Darwin)
            if command -v brew &> /dev/null; then
                brew install llvm
                return $?
            fi
            xcode-select --install >/dev/null 2>&1 || true
            return 1
            ;;
        Linux)
            if command -v apt-get &> /dev/null; then
                run_privileged apt-get update
                run_privileged apt-get install -y build-essential pkg-config
            elif command -v dnf &> /dev/null; then
                run_privileged dnf install -y gcc gcc-c++ make pkgconf-pkg-config
            elif command -v yum &> /dev/null; then
                run_privileged yum install -y gcc gcc-c++ make pkgconfig
            elif command -v pacman &> /dev/null; then
                run_privileged pacman -Sy --noconfirm base-devel pkgconf
            elif command -v zypper &> /dev/null; then
                run_privileged zypper --non-interactive install -y gcc gcc-c++ make pkg-config
            elif command -v apk &> /dev/null; then
                run_privileged apk add --no-cache build-base pkgconf
            else
                return 1
            fi
            ;;
        *)
            return 1
            ;;
    esac

    command -v cc &> /dev/null
}

# Main script
log_header "Void Scripting Runtime Installer"
load_rust_env

# Check if git is installed
log_step "Checking prerequisites"
log_progress "Checking Git..."
if ! command -v git &> /dev/null; then
    log_warning "Git is not installed"
    if confirm "Install Git automatically now?"; then
        log_progress "Installing Git..."
        if ! install_git_auto; then
            log_error "Failed to auto-install Git"
            if [ "$(uname -s)" = "Darwin" ]; then
                log_info "On macOS, install Homebrew + git, or run xcode-select --install."
            fi
            log_info "Install Git manually: https://git-scm.com/download"
            exit 1
        fi
        log_success "Git installed successfully"
    else
        if [ "$?" -eq 2 ]; then
            log_error "Could not prompt for Git install confirmation (no interactive terminal)."
            log_info "Run this script in an interactive terminal, or install Git first."
        fi
        log_error "Git is required to install Void."
        log_info "Install Git first: https://git-scm.com/download"
        exit 1
    fi
fi
log_success "Git found"

# Check if Rust is installed
log_progress "Checking Rust..."
if ! command -v rustc &> /dev/null; then
    log_warning "Rust is not installed"
    if confirm "Install Rust automatically now?"; then
        if ! command -v curl &> /dev/null; then
            log_error "curl is required to auto-install Rust"
            log_info "Please install curl, then re-run the installer."
            exit 1
        fi
        echo -e "${CYAN}Installing Rust...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        load_rust_env
        if ! command -v rustc &> /dev/null; then
            log_error "Rust installation finished, but rustc is still not in PATH"
            log_info "Open a new shell or source ~/.cargo/env, then run installer again."
            exit 1
        fi
        log_success "Rust installed successfully"
    else
        if [ "$?" -eq 2 ]; then
            log_error "Could not prompt for Rust install confirmation (no interactive terminal)."
            log_info "Run this script in an interactive terminal, or install Rust first."
        fi
        log_error "Rust is required to install Void."
        log_info "Install Rust first: https://rustup.rs"
        exit 1
    fi
else
    log_success "Rust found"
fi

log_progress "Checking C toolchain (cc)..."
if ! command -v cc &> /dev/null; then
    log_warning "C compiler (cc) is not installed"
    if confirm "Install C build tools automatically now?"; then
        log_progress "Installing C build tools..."
        if ! install_c_toolchain_auto; then
            log_error "Failed to auto-install C build tools"
            if [ "$(uname -s)" = "Darwin" ]; then
                log_info "On macOS, install Command Line Tools (xcode-select --install)."
            fi
            log_info "Install a C toolchain manually (gcc/clang + make), then re-run."
            exit 1
        fi
        log_success "C build tools installed successfully"
    else
        if [ "$?" -eq 2 ]; then
            log_error "Could not prompt for C toolchain confirmation (no interactive terminal)."
            log_info "Run this script in an interactive terminal, or install build tools first."
        fi
        log_error "A C compiler is required to build Void dependencies."
        log_info "Install build tools manually, then re-run installer."
        exit 1
    fi
else
    log_success "C compiler found"
fi

log_info "Node.js is not required. Void and VPM build from Rust only."

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
    if ! cargo build --release; then
        log_error "Language runtime build failed"
        exit 1
    fi
    log_success "Language runtime built"
else
    log_warning "Cargo.toml not found in language directory"
fi

cd "$INSTALL_DIR/void"

log_separator

# Build the package manager
log_step "Building package manager (VPM)"
cd package-manager

if [ -f "Cargo.toml" ]; then
    log_progress "Compiling Rust package manager binaries..."
    if ! cargo build --release --manifest-path "$INSTALL_DIR/void/package-manager/Cargo.toml" -p vpm -p void-registry; then
        log_error "Package manager build failed"
        exit 1
    fi
    log_success "Package manager built"
else
    log_warning "Cargo.toml not found in package-manager directory"
fi

cd "$INSTALL_DIR/void"

log_separator

# Create symlinks for easy access
log_step "Setting up command shortcuts"

mkdir -p "$HOME/.local/bin"

# Create void executable symlink (use stable launcher script, not target path)
if [ -f "$INSTALL_DIR/void/language/void" ]; then
    chmod +x "$INSTALL_DIR/void/language/void" >/dev/null 2>&1 || true
    ln -sf "$INSTALL_DIR/void/language/void" "$HOME/.local/bin/void"
    log_success "'void' command linked"
else
    log_warning "Void launcher not found"
fi

# Create vpm executable symlink
if [ -f "$INSTALL_DIR/void/package-manager/bin/vpm" ]; then
    ln -sf "$INSTALL_DIR/void/package-manager/bin/vpm" "$HOME/.local/bin/vpm"
    log_success "'vpm' command linked"
else
    log_warning "vpm launcher not found"
fi

# Create registry executable symlink
if [ -f "$INSTALL_DIR/void/package-manager/bin/void-registry" ]; then
    ln -sf "$INSTALL_DIR/void/package-manager/bin/void-registry" "$HOME/.local/bin/void-registry"
    log_success "'void-registry' command linked"
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
echo -e "  ${CYAN}void $INSTALL_DIR/void/language/examples/hyperdrive.void${NC}  Run advanced example"

echo -e "\n${GREEN}${BOLD}Installation Location:${NC}"
echo -e "  ${CYAN}$INSTALL_DIR${NC}"

echo -e "\n${GREEN}${BOLD}Documentation:${NC}"
echo -e "  ${CYAN}https://github.com/Olibot1107/void${NC}"

echo -e "\n${GREEN}${BOLD}Enjoy coding with Void! 🚀${NC}\n"
