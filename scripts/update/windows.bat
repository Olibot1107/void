@echo off
REM Void Runtime Update Script for Windows
REM Updates Void to the latest version

setlocal enabledelayedexpansion

cls

echo.
echo ╔════════════════════════════════════════╗
echo ║  Void Runtime Update                  ║
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

REM Determine installation directory
if not defined VOID_INSTALL_DIR (
    set "VOID_INSTALL_DIR=%USERPROFILE%\.void"
)

REM Check if Void is installed
if not exist "!VOID_INSTALL_DIR!\void" (
    color 4F
    echo %ERROR% Void is not installed at !VOID_INSTALL_DIR!
    echo.
    color 0B
    echo %INFO% Run the installer first: install.bat
    color 07
    pause
    exit /b 1
)

echo %ARROW% Update
echo.
color 0B
echo %INFO% Installation directory: !VOID_INSTALL_DIR!
color 07

echo.
echo ────────────────────────────────────────
echo.

REM Update repository
echo %ARROW% Updating repository
echo.

cd /d "!VOID_INSTALL_DIR!\void"

color 0B
echo %PROGRESS% Fetching latest changes...
color 07

git pull origin main >nul 2>&1
if errorlevel 1 (
    color 0E
    echo %WARNING% Repository update encountered issues
    color 07
) else (
    color 0A
    echo %SUCCESS% Repository updated
    color 07
)

echo.
echo ────────────────────────────────────────
echo.

REM Rebuild language runtime
echo %ARROW% Rebuilding language runtime
echo.

cd /d "!VOID_INSTALL_DIR!\void\language"

color 0B
echo %PROGRESS% Compiling Rust code ^(this may take a while^)...
color 07
echo.

cargo build --release

if errorlevel 1 (
    color 0E
    echo %WARNING% Build completed with issues
    color 07
) else (
    color 0A
    echo %SUCCESS% Language runtime built
    color 07
)

echo.
echo ────────────────────────────────────────
echo.

REM Rebuild package manager
cd /d "!VOID_INSTALL_DIR!\void"
echo %ARROW% Rebuilding package manager ^(VPM^)
echo.

cd /d "!VOID_INSTALL_DIR!\void\package-manager"

color 0B
echo %PROGRESS% Installing npm dependencies...
color 07

call npm install

if errorlevel 1 (
    color 0E
    echo %WARNING% npm install encountered issues
    color 07
) else (
    color 0A
    echo %SUCCESS% Dependencies installed
    color 07
)

color 0B
echo %PROGRESS% Building package manager...
color 07

call npm run build

if errorlevel 1 (
    color 0E
    echo %WARNING% Build completed with issues
    color 07
) else (
    color 0A
    echo %SUCCESS% Package manager built
    color 07
)

cd /d "!VOID_INSTALL_DIR!\void"

echo.
echo ────────────────────────────────────────
echo.

REM Verify update
echo %ARROW% Verifying update
echo.

if exist "!VOID_INSTALL_DIR!\void\language\target\release\void.exe" (
    color 0A
    echo %SUCCESS% Void is installed and accessible
    color 07
) else (
    color 0E
    echo %WARNING% Void executable not found
    color 07
)

echo.
echo ╔════════════════════════════════════════╗
echo ║   Update Complete!                    ║
echo ╚════════════════════════════════════════╝
echo.

color 0A
echo Next steps:
echo.
color 0B
echo   void.exe "!VOID_INSTALL_DIR!\void\language\examples\hello.void" - Test with hello world
echo   void.exe "!VOID_INSTALL_DIR!\void\language\examples\hyperdrive.void" - Test advanced example
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
echo Happy coding! Running latest Void
color 07
echo.

pause
