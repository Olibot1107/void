# Void Runtime - Update Guide

> Keep your Void installation up to date with the latest features and fixes.

---

## Quick Update

### Linux/macOS

#### Option 1: Using curl (Recommended)
```bash
curl -sSL https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/update/linux.sh | bash
```

#### Option 2: Manual update
```bash
cd ~/.local/void/void
git pull origin main
cd language && cargo build --release
cd ../package-manager && cargo build --release -p vpm -p void-registry
```

#### Option 3: Custom installation directory
```bash
VOID_INSTALL_DIR=$HOME/my-void-dir curl -sSL https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/update/linux.sh | bash
```

---

### Windows

#### Option 1: Using PowerShell (Recommended)
```powershell
powershell -Command "irm https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/update/windows.bat | iex"
```

#### Option 2: Manual update
```cmd
cd %USERPROFILE%\.void\void
git pull origin main
cd language
cargo build --release
cd ..\package-manager
cargo build --release -p vpm -p void-registry
```

#### Option 3: Custom installation directory
```cmd
set VOID_INSTALL_DIR=C:\custom-path
powershell -Command "irm https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/update/windows.bat | iex"
```

---

## What the Update Does

The update script automatically:
- Checks if Void is installed
- Fetches the latest changes from GitHub
- Rebuilds the language runtime
- Rebuilds the package manager
- Verifies the installation
- Reports the status

---

## Prerequisites

- **Git** - for pulling latest changes
- **Rust** - for building the runtime (should already be installed)
- **Node.js** - not required

---

## Checking Your Version

Before updating, check your current version:

```bash
void --version
vpm --version
~/.local/void/void/package-manager/bin/void-registry --version
```

Or see what's new in the repository:

```bash
cd ~/.local/void/void
git log --oneline -n 10
```

---

## After Updating

After a successful update, verify everything works:

```bash
# Test the runtime
void ~/.local/void/void/language/examples/hello.void
void ~/.local/void/void/language/examples/hyperdrive.void

# Test the package manager
vpm --help
vpm --version
~/.local/void/void/package-manager/bin/void-registry --version
```

---

## Troubleshooting

### Update Failed

If the update fails, try these steps:

1. If you changed local files, stash or commit first:
```bash
cd ~/.local/void/void
git status
git stash push -u -m "void-update-temp"
```

2. Check Git status:
```bash
cd ~/.local/void/void
git status
```

3. Reset to clean state (destructive):
```bash
git reset --hard origin/main
```

4. Clear build cache and rebuild:
```bash
cd language
cargo clean
cargo build --release
cd ../package-manager
cargo clean
cargo build --release -p vpm -p void-registry
```

### Build Errors

If you encounter build errors:

1. Update Rust:
```bash
rustup update
```

2. Rebuild package-manager binaries:
```bash
cd ~/.local/void/void/package-manager
cargo build --release -p vpm -p void-registry
```

### Command Not Working After Update

Restart your terminal or reload your shell:

```bash
# For bash
source ~/.bashrc

# For zsh
source ~/.zshrc

# For Windows
Restart Command Prompt or PowerShell
```

---

## Automatic Updates

To schedule regular updates (Linux/macOS):

Add to crontab to update daily at 2 AM:
```bash
crontab -e

# Add this line:
0 2 * * * $HOME/path-to-void/update.sh >> $HOME/void-update.log 2>&1
```

---

## Rollback to Previous Version

If an update breaks something, you can revert:

```bash
cd ~/.local/void/void
git log --oneline

# Find the commit you want to revert to
git checkout <commit-hash>

# Rebuild
cd language && cargo build --release
cd ../package-manager && cargo build --release -p vpm -p void-registry
```

---

## Checking for Updates

Check if updates are available without installing:

```bash
cd ~/.local/void/void
git fetch origin
git log HEAD..origin/main --oneline
```

If there are any lines in the output, updates are available.

---

## Update History

View recent changes:

```bash
cd ~/.local/void/void
git log --oneline -n 20
```

See details of a specific commit:
```bash
git show <commit-hash>
```

---

## Documentation

- Repository: https://github.com/Olibot1107/void
- Language Docs: `~/.local/void/void/language/README.md`
- Package Manager: `~/.local/void/void/package-manager/README.md`
- Uninstall Guide: `~/.local/void/void/Uninstall.md`

---

## Updating Dependencies

If you need to update dependencies separately:

```bash
# Update Rust toolchain
rustup update

# Rebuild package manager binaries
cd ~/.local/void/void/package-manager
cargo build --release -p vpm -p void-registry
```

---

**Keep your Void installation current and secure**
