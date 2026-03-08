@echo off
REM Void Runtime Uninstaller for Windows

setlocal enabledelayedexpansion

cls
echo.
echo ╔════════════════════════════════════════╗
echo ║  Void Runtime Uninstaller             ║
echo ╚════════════════════════════════════════╝
echo.

set "SUCCESS=[✓]"
set "ERROR=[✗]"
set "WARNING=[⚠]"
set "INFO=[ℹ]"

if not defined VOID_INSTALL_DIR (
    set "VOID_INSTALL_DIR=%USERPROFILE%\.void"
)

set "BIN_DIR=%USERPROFILE%\.void\bin"
set "VOID_BIN=%BIN_DIR%\void.bat"
set "VPM_BIN=%BIN_DIR%\vpm.bat"
set "VOID_LAB_DIR=%USERPROFILE%\.void-lab"

echo %INFO% Installation directory: !VOID_INSTALL_DIR!
echo %INFO% Command shortcuts: !BIN_DIR!
if "%VOID_REMOVE_VOID_LAB%"=="1" (
    echo %INFO% Will also remove: !VOID_LAB_DIR!
)
echo.

if not "%VOID_UNINSTALL_FORCE%"=="1" (
    set /p "CONFIRM=This will remove Void from your machine. Continue? (y/N): "
    if /I not "!CONFIRM!"=="y" (
        echo.
        echo %WARNING% Uninstall cancelled.
        pause
        exit /b 0
    )
)

if exist "!VOID_BIN!" (
    del /f /q "!VOID_BIN!"
    echo %SUCCESS% Removed command: !VOID_BIN!
) else (
    echo %INFO% Command not found: !VOID_BIN!
)

if exist "!VPM_BIN!" (
    del /f /q "!VPM_BIN!"
    echo %SUCCESS% Removed command: !VPM_BIN!
) else (
    echo %INFO% Command not found: !VPM_BIN!
)

if exist "!VOID_INSTALL_DIR!" (
    rmdir /s /q "!VOID_INSTALL_DIR!"
    echo %SUCCESS% Removed installation directory: !VOID_INSTALL_DIR!
) else (
    echo %INFO% Installation directory not found: !VOID_INSTALL_DIR!
)

if "%VOID_REMOVE_VOID_LAB%"=="1" (
    if exist "!VOID_LAB_DIR!" (
        rmdir /s /q "!VOID_LAB_DIR!"
        echo %SUCCESS% Removed lab output directory: !VOID_LAB_DIR!
    ) else (
        echo %INFO% Lab output directory not found: !VOID_LAB_DIR!
    )
)

echo.
echo %SUCCESS% Uninstall complete.
echo %INFO% If you manually added PATH entry, you can remove %%USERPROFILE%%\.void\bin from user PATH.
echo.
pause
