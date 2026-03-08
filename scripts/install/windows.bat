@echo off
REM Void Runtime Installer for Windows
REM This script automatically clones and sets up the Void scripting runtime

setlocal enabledelayedexpansion

cls

REM Color codes using default Windows colors
REM We'll use title, echo with special chars, and cls for visual appeal

echo.
echo ╔════════════════════════════════════════╗
echo ║  Void Scripting Runtime Installer     ║
echo ║  [v1.0 - Windows Edition]             ║
echo ╚════════════════════════════════════════╝
echo.

REM Logging functions
set "SUCCESS=[✓]"
set "ERROR=[✗]"
set "WARNING=[⚠]"
set "INFO=[ℹ]"
set "PROGRESS=[⟳]"
set "ARROW=[→]"

REM Check if Git is installed
echo %ARROW% Checking prerequisites...
echo.

git --version >nul 2>&1
if errorlevel 1 (
    color 0E
    echo %WARNING% Git is not installed or not in PATH.
    echo.
    set /p GIT_INSTALL_CHOICE=Install Git automatically now? [Y/N]: 
    if /I "!GIT_INSTALL_CHOICE!"=="N" (
        color 4F
        echo %ERROR% Git is required to install Void.
        echo Please install Git first: https://git-scm.com/download/win
        color 07
        pause
        exit /b 1
    )
    if not "!GIT_INSTALL_CHOICE!"=="" if /I not "!GIT_INSTALL_CHOICE!"=="Y" (
        color 4F
        echo %ERROR% Invalid choice. Please run installer again and answer Y or N.
        color 07
        pause
        exit /b 1
    )

    where winget >nul 2>&1
    if errorlevel 1 (
        color 4F
        echo %ERROR% Automatic Git install requires winget.
        echo Install Git manually: https://git-scm.com/download/win
        color 07
        pause
        exit /b 1
    )

    color 0B
    echo %PROGRESS% Installing Git with winget...
    color 07
    winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
    if errorlevel 1 (
        color 4F
        echo %ERROR% Failed to install Git with winget
        echo Install Git manually: https://git-scm.com/download/win
        color 07
        pause
        exit /b 1
    )

    if exist "%ProgramFiles%\Git\cmd\git.exe" (
        set "PATH=%ProgramFiles%\Git\cmd;%PATH%"
    )
    if exist "%ProgramFiles(x86)%\Git\cmd\git.exe" (
        set "PATH=%ProgramFiles(x86)%\Git\cmd;%PATH%"
    )

    git --version >nul 2>&1
    if errorlevel 1 (
        color 4F
        echo %ERROR% Git was installed but is not available in this shell yet.
        echo Open a new terminal and run installer again.
        color 07
        pause
        exit /b 1
    )

)
color 0A
echo %SUCCESS% Git found
color 07

REM Check if Rust is installed
rustc --version >nul 2>&1
if errorlevel 1 (
    color 0E
    echo %WARNING% Rust is not installed. Void requires Rust to build.
    echo.
    set /p RUST_INSTALL_CHOICE=Install Rust automatically now? [Y/N]: 
    if /I "!RUST_INSTALL_CHOICE!"=="N" (
        color 4F
        echo %ERROR% Rust is required to install Void.
        echo Please install Rust first: https://rustup.rs
        color 07
        pause
        exit /b 1
    )
    if not "!RUST_INSTALL_CHOICE!"=="" if /I not "!RUST_INSTALL_CHOICE!"=="Y" (
        color 4F
        echo %ERROR% Invalid choice. Please run installer again and answer Y or N.
        color 07
        pause
        exit /b 1
    )

    echo %PROGRESS% Installing Rust...
    color 07
    echo.

    powershell -Command "(New-Object System.Net.ServicePointManager).SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://win.rustup.rs/x86_64'))"

    if errorlevel 1 (
        color 4F
        echo %ERROR% Failed to install Rust
        color 07
        pause
        exit /b 1
    )
    rustc --version >nul 2>&1
    if errorlevel 1 (
        color 4F
        echo %ERROR% Rust installation completed, but rustc was not found in PATH.
        echo Open a new terminal and run installer again.
        color 07
        pause
        exit /b 1
    )
    color 0A
    echo %SUCCESS% Rust installed successfully
    color 07
) else (
    color 0A
    echo %SUCCESS% Rust found
    color 07
)

color 0B
echo %INFO% Node.js is not required. Void and VPM are Rust binaries.
color 07

echo.
echo ────────────────────────────────────────
echo.

REM Set installation directory
if not defined VOID_INSTALL_DIR (
    set "VOID_INSTALL_DIR=%USERPROFILE%\.void"
)

echo %ARROW% Setup
echo.
color 0B
echo %INFO% Installation directory: %VOID_INSTALL_DIR%
color 07

REM Create installation directory if it doesn't exist
if not exist "!VOID_INSTALL_DIR!" (
    mkdir "!VOID_INSTALL_DIR!"
)
color 0A
echo %SUCCESS% Installation directory ready
color 07

echo.
echo ────────────────────────────────────────
echo.

REM Clone the repository
echo %ARROW% Cloning repository
echo.

if exist "!VOID_INSTALL_DIR!\void" (
    color 0E
    echo %PROGRESS% Void directory exists, updating...
    color 07
    cd /d "!VOID_INSTALL_DIR!\void"
    git pull origin main >nul 2>&1
    if errorlevel 1 (
        color 0E
        echo %WARNING% Git update encountered issues
        color 07
    )
) else (
    color 0B
    echo %PROGRESS% Cloning from GitHub...
    color 07
    git clone https://github.com/Olibot1107/void.git "!VOID_INSTALL_DIR!\void" >nul 2>&1
    if errorlevel 1 (
        color 4F
        echo %ERROR% Failed to clone repository
        color 07
        pause
        exit /b 1
    )
)

cd /d "!VOID_INSTALL_DIR!\void"
color 0A
echo %SUCCESS% Repository ready
color 07

echo.
echo ────────────────────────────────────────
echo.

REM Build the language runtime
echo %ARROW% Building language runtime
echo.

cd /d "!VOID_INSTALL_DIR!\void\language"

if exist "Cargo.toml" (
    color 0B
    echo %PROGRESS% Compiling Rust code ^(this may take a while^)...
    color 07
    echo.
    cargo build --release
    if errorlevel 1 (
        color 0E
        echo %WARNING% Build encountered issues
        color 07
    ) else (
        color 0A
        echo %SUCCESS% Language runtime built
        color 07
    )
) else (
    color 0E
    echo %WARNING% Cargo.toml not found in language directory
    color 07
)

echo.
echo ────────────────────────────────────────
echo.

REM Build the package manager
cd /d "!VOID_INSTALL_DIR!\void"
echo %ARROW% Building package manager ^(VPM^)
echo.

cd /d "!VOID_INSTALL_DIR!\void\package-manager"

if exist "Cargo.toml" (
    color 0B
    echo %PROGRESS% Compiling Rust package manager binaries...
    color 07
    cargo build --release --manifest-path "!VOID_INSTALL_DIR!\void\package-manager\Cargo.toml" -p vpm -p void-registry
    if errorlevel 1 (
        color 0E
        echo %WARNING% Package manager build encountered issues
        color 07
    ) else (
        color 0A
        echo %SUCCESS% Package manager built
        color 07
    )
) else (
    color 0E
    echo %WARNING% Cargo.toml not found in package-manager directory
    color 07
)

cd /d "!VOID_INSTALL_DIR!\void"

echo.
echo ────────────────────────────────────────
echo.

REM Create batch scripts for easy access
echo %ARROW% Setting up command shortcuts
echo.

if not exist "%USERPROFILE%\.void\bin" (
    mkdir "%USERPROFILE%\.void\bin"
)

REM Create void.bat shortcut
if exist "!VOID_INSTALL_DIR!\void\language\target\release\void.exe" (
    (
        echo @echo off
        echo "!VOID_INSTALL_DIR!\void\language\target\release\void.exe" %%*
    ) > "%USERPROFILE%\.void\bin\void.bat"
    color 0A
    echo %SUCCESS% 'void' command created
    color 07
) else (
    color 0E
    echo %WARNING% Void executable not found
    color 07
)

REM Create vpm.bat shortcut
if exist "!VOID_INSTALL_DIR!\void\package-manager\target\release\vpm.exe" (
    (
        echo @echo off
        echo "!VOID_INSTALL_DIR!\void\package-manager\target\release\vpm.exe" %%*
    ) > "%USERPROFILE%\.void\bin\vpm.bat"
    color 0A
    echo %SUCCESS% 'vpm' command created
    color 07
) else (
    color 0E
    echo %WARNING% VPM executable not found
    color 07
)

REM Create void-registry.bat shortcut
if exist "!VOID_INSTALL_DIR!\void\package-manager\target\release\void-registry.exe" (
    (
        echo @echo off
        echo "!VOID_INSTALL_DIR!\void\package-manager\target\release\void-registry.exe" %%*
    ) > "%USERPROFILE%\.void\bin\void-registry.bat"
    color 0A
    echo %SUCCESS% 'void-registry' command created
    color 07
)

echo.
echo ────────────────────────────────────────
echo.

REM Check and update PATH
echo %ARROW% Updating system PATH
echo.

for /f "tokens=2*" %%A in ('reg query "HKCU\Environment" /v PATH 2^>nul') do set "USERPATH=%%B"

if not "!USERPATH!"=="" (
    echo !USERPATH! | find /i "%USERPROFILE%\.void\bin" >nul
    if errorlevel 1 (
        echo %PROGRESS% Adding to user PATH...
        setx PATH "!USERPATH!;%USERPROFILE%\.void\bin" >nul 2>&1
        color 0A
        echo %SUCCESS% PATH updated
        color 0E
        echo %WARNING% Restart command prompt for changes to take effect
        color 07
    ) else (
        color 0A
        echo %SUCCESS% PATH is already configured
        color 07
    )
) else (
    echo %PROGRESS% Setting up PATH...
    setx PATH "%USERPROFILE%\.void\bin" >nul 2>&1
    color 0A
    echo %SUCCESS% PATH configured
    color 0E
    echo %WARNING% Restart command prompt for changes to take effect
    color 07
)

echo.
echo ────────────────────────────────────────
echo.

REM Test installation
echo %ARROW% Verifying installation
echo.

if exist "!VOID_INSTALL_DIR!\void\language\target\release\void.exe" (
    color 0A
    echo %SUCCESS% Void is installed successfully!
    color 07
) else (
    color 0E
    echo %WARNING% Void executable not found. Build may have failed.
    color 07
)

echo.
echo ╔════════════════════════════════════════╗
echo ║   ✨ Installation Complete! ✨        ║
echo ╚════════════════════════════════════════╝
echo.

color 0A
echo Quick Start:
echo.
color 0B
echo   void.exe "!VOID_INSTALL_DIR!\void\language\examples\hello.void" - Run hello world
echo   void.exe "!VOID_INSTALL_DIR!\void\language\examples\main.void"  - Run main example
echo   void.exe "!VOID_INSTALL_DIR!\void\language\examples\hyperdrive.void" - Run advanced example
color 07
echo.

color 0A
echo Installation Location:
echo.
color 0B
echo   !VOID_INSTALL_DIR!
color 07
echo.

color 0A
echo Documentation:
echo.
color 0B
echo   https://github.com/Olibot1107/void
color 07
echo.

color 0A
echo Enjoy coding with Void! 🚀
color 07
echo.

pause
