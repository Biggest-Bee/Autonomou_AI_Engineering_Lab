@echo off
REM Build script for Autonomous AI Engineering Lab
REM This script builds the frontend and then builds the Tauri application

REM Change to the frontend directory
cd /d "%~dp0\..\frontend"

REM Build the frontend
echo Building frontend...
call npm run build
if errorlevel 1 (
    echo Frontend build failed!
    exit /b 1
)

REM Change back to src-tauri directory
cd /d "%~dp0"

REM Build the Tauri application
echo Building Tauri application...
call cargo tauri build
if errorlevel 1 (
    echo Tauri build failed!
    exit /b 1
)

echo Build completed successfully!
pause
