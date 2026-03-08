#!/bin/bash

# Void Runtime Uninstaller for Linux/macOS

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

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

log_separator() {
    echo -e "${CYAN}─────────────────────────────────────────${NC}"
}

log_header "Void Runtime Uninstaller"

INSTALL_DIR="${VOID_INSTALL_DIR:=$HOME/.local/void}"
BIN_DIR="$HOME/.local/bin"
VOID_BIN="$BIN_DIR/void"
VPM_BIN="$BIN_DIR/vpm"
VOID_LAB_DIR="$HOME/.void-lab"

log_info "Installation directory: ${BOLD}$INSTALL_DIR${NC}"
log_info "Command shortcuts: ${BOLD}$BIN_DIR${NC}"
if [[ "${VOID_REMOVE_VOID_LAB:-0}" == "1" ]]; then
    log_info "Will also remove: ${BOLD}$VOID_LAB_DIR${NC}"
fi

log_separator

if [[ "${VOID_UNINSTALL_FORCE:-0}" != "1" ]]; then
    read -r -p "This will remove Void from your machine. Continue? (y/N): " CONFIRM
    if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
        log_warning "Uninstall cancelled."
        exit 0
    fi
fi

if [[ -e "$VOID_BIN" || -L "$VOID_BIN" ]]; then
    rm -f "$VOID_BIN"
    log_success "Removed command: $VOID_BIN"
else
    log_info "Command not found: $VOID_BIN"
fi

if [[ -e "$VPM_BIN" || -L "$VPM_BIN" ]]; then
    rm -f "$VPM_BIN"
    log_success "Removed command: $VPM_BIN"
else
    log_info "Command not found: $VPM_BIN"
fi

if [[ -d "$INSTALL_DIR" ]]; then
    rm -rf "$INSTALL_DIR"
    log_success "Removed installation directory: $INSTALL_DIR"
else
    log_info "Installation directory not found: $INSTALL_DIR"
fi

if [[ "${VOID_REMOVE_VOID_LAB:-0}" == "1" ]]; then
    if [[ -d "$VOID_LAB_DIR" ]]; then
        rm -rf "$VOID_LAB_DIR"
        log_success "Removed lab output directory: $VOID_LAB_DIR"
    else
        log_info "Lab output directory not found: $VOID_LAB_DIR"
    fi
fi

log_separator
log_header "Uninstall Complete"

log_info "If you manually edited shell config for PATH, you can remove:"
echo -e "${GREEN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
