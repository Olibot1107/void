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
cd ~/.local/void
git pull origin main
cd language && cargo build --release
cd ../package-manager && npm install && npm run build
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
npm install
npm run build
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
- **Node.js** - for building the package manager (should already be installed)

---

## Checking Your Version

Before updating, check your current version:

```bash
void --version
```

Or see what's new in the repository:

```bash
cd ~/.local/void
git log --oneline -n 10
```

---

## After Updating

After a successful update, verify everything works:

```bash
# Test the runtime
void ~/.local/void/examples/hello.void

# Test the package manager
vpm --help

# Start the registry
cd ~/.local/void/package-manager
./bin/void-registry
```

---

## Troubleshooting

### Update Failed

If the update fails, try these steps:

1. Check Git status:
```bash
cd ~/.local/void
git status
```

2. Reset to clean state:
```bash
git reset --hard origin/main
```

3. Clear build cache and rebuild:
```bash
cd language
cargo clean
cargo build --release
cd ../package-manager
rm -rf node_modules
npm install
npm run build
```

### Build Errors

If you encounter build errors:

1. Update Rust:
```bash
rustup update
```

2. Check Node.js version:
```bash
node --version
npm --version
```

3. Clear npm cache:
```bash
npm cache clean --force
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
cd ~/.local/void
git log --oneline

# Find the commit you want to revert to
git checkout <commit-hash>

# Rebuild
cd language && cargo build --release
cd ../package-manager && npm install && npm run build
```

---

## Checking for Updates

Check if updates are available without installing:

```bash
cd ~/.local/void
git fetch origin
git log HEAD..origin/main --oneline
```

If there are any lines in the output, updates are available.

---

## Update History

View recent changes:

```bash
cd ~/.local/void
git log --oneline -n 20
```

See details of a specific commit:
```bash
git show <commit-hash>
```

---

## Documentation

- Repository: https://github.com/Olibot1107/void
- Language Docs: `~/.local/void/language/README.md`
- Package Manager: `~/.local/void/package-manager/README.md`

---

## Updating Dependencies

If you need to update dependencies separately:

```bash
# Update Rust toolchain
rustup update

# Update npm packages
cd ~/.local/void/package-manager
npm update

# Update Node.js
# Visit https://nodejs.org for the latest version
```

---

**Keep your Void installation current and secure**