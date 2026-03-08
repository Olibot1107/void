# Void Runtime - Uninstall Guide

> Remove Void and its command shortcuts from your machine.

---

## Quick Uninstall

### Linux/macOS

#### Option 1: Using curl (Recommended)
```bash
curl -sSL https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/linux.sh | bash
```

#### Option 2: Manual run
```bash
cd ~/.local/void/void
bash ./scripts/uninstall/linux.sh
```

#### Option 3: Force mode (no prompt)
```bash
curl -sSL https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/linux.sh | VOID_UNINSTALL_FORCE=1 bash
```

#### Option 4: Custom install directory
```bash
curl -sSL https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/linux.sh | VOID_INSTALL_DIR=$HOME/my-void-dir bash
```

#### Optional: remove local Hyperdrive output
```bash
curl -sSL https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/linux.sh | VOID_REMOVE_VOID_LAB=1 bash
```

---

### Windows

#### Option 1: Using PowerShell (Recommended)
```powershell
powershell -Command "$u='https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/windows.bat'; $p='$env:TEMP\void-uninstall.bat'; irm $u -OutFile $p; cmd /c $p"
```

#### Option 2: Manual run
```cmd
cd %USERPROFILE%\.void\void
scripts\uninstall\windows.bat
```

#### Option 3: Force mode (no prompt)
```cmd
set VOID_UNINSTALL_FORCE=1
powershell -Command "$u='https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/windows.bat'; $p='$env:TEMP\void-uninstall.bat'; irm $u -OutFile $p; cmd /c $p"
```

#### Option 4: Custom install directory
```cmd
set VOID_INSTALL_DIR=C:\custom-path
powershell -Command "$u='https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/windows.bat'; $p='$env:TEMP\void-uninstall.bat'; irm $u -OutFile $p; cmd /c $p"
```

#### Optional: remove local Hyperdrive output
```cmd
set VOID_REMOVE_VOID_LAB=1
powershell -Command "$u='https://raw.githubusercontent.com/Olibot1107/void/refs/heads/main/scripts/uninstall/windows.bat'; $p='$env:TEMP\void-uninstall.bat'; irm $u -OutFile $p; cmd /c $p"
```

---

## What Gets Removed

- Installed Void directory (`~/.local/void` by default on Linux/macOS)
- Command shortcuts (`~/.local/bin/void`, `~/.local/bin/vpm` on Linux/macOS)
- Command shortcuts (`%USERPROFILE%\.void\bin\void.bat`, `%USERPROFILE%\.void\bin\vpm.bat` on Windows)
- Optional Hyperdrive output directory (`~/.void-lab` / `%USERPROFILE%\.void-lab`) when `VOID_REMOVE_VOID_LAB=1`

---

## Notes

- Rust and Git are not removed.
- If you manually edited PATH, remove the Void path entry yourself.
