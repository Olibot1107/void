# Void Runtime - Installer

> A fast scripting runtime written in Rust with an integrated package ecosystem.

---

## Installation

### Linux/macOS

#### Option 1: Using curl (Recommended)
```bash
curl -sSL https://raw.githubusercontent.com/Olibot1107/void/main/install.sh | bash
```

#### Option 2: Manual download and run
```bash
chmod +x install.sh
./install.sh
```

#### Option 3: Custom installation directory
```bash
VOID_INSTALL_DIR=$HOME/my-void-dir ./install.sh
```

---

### Windows

#### Option 1: Using PowerShell (Recommended)
```powershell
powershell -Command "irm https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/install/windows.bat | iex"
```

#### Option 2: Direct execution
Simply double-click `install.bat` or run from Command Prompt:
```cmd
install.bat
```

#### Option 3: From PowerShell
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
.\install.bat
```

#### Option 4: Custom installation directory
```cmd
set VOID_INSTALL_DIR=C:\custom-path
install.bat
```

---

## What Gets Installed

The installer automatically:
- Checks for required dependencies (Git, Rust, Node.js)
- Installs Rust if missing
- Clones the Void repository
- Builds the language runtime
- Builds the VPM package manager
- Creates command-line shortcuts
- Updates system PATH
- Verifies the installation

---

## Prerequisites

- **Git** - for cloning the repository
- **Rust** - required for building (auto-installed if missing)
- **Node.js** (optional) - for package manager features

---

## Default Installation Locations

### Linux/macOS:
```
~/.local/void/           # Installation root
~/.local/void/void/      # Cloned Void repository
~/.local/bin/void        # Runtime command
~/.local/bin/vpm         # Package manager command
```

### Windows:
```
%USERPROFILE%\.void\                    # Installation root
%USERPROFILE%\.void\void\               # Cloned Void repository
%USERPROFILE%\.void\bin\void.bat        # Runtime command
%USERPROFILE%\.void\bin\vpm.bat         # Package manager command
```

---

## After Installation

Try these commands:

```bash
# Run examples
void ~/.local/void/void/language/examples/hello.void
void ~/.local/void/void/language/examples/hyperdrive.void

# Initialize a new project
vpm
vpm init my_project

# Search packages
vpm search util --registry http://127.0.0.1:4090
```

`vpm` now defaults to install mode help when run without arguments.

---

## Documentation

- Repository: https://github.com/Olibot1107/void
- Language Docs: `~/.local/void/void/language/README.md`
- Package Manager: `~/.local/void/void/package-manager/README.md`

---

## Troubleshooting

### Command not found: void or vpm

**Linux/macOS:**
Add to `~/.bashrc` or `~/.zshrc`:
```bash
export PATH="$HOME/.local/bin:$PATH"
```

Then reload:
```bash
source ~/.bashrc
```

**Windows:**
Restart Command Prompt or PowerShell after installation.

### Build Issues

Update and reinstall Rust:
```bash
rustup update
cargo build --release
```

---

**Ready to start coding with Void**
