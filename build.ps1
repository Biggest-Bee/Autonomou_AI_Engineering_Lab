# Build script for Autonomous AI Engineering Lab
# This script builds the frontend and then builds the Tauri application

Write-Host "Building Autonomous AI Engineering Lab..." -ForegroundColor Green

# Change to the frontend directory
Set-Location -Path "$PSScriptRoot\..\frontend"

# Build the frontend
Write-Host "Building frontend..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Frontend build failed!" -ForegroundColor Red
    exit 1
}

# Change back to src-tauri directory
Set-Location -Path "$PSScriptRoot"

# Build the Tauri application
Write-Host "Building Tauri application..." -ForegroundColor Yellow
cargo tauri build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Tauri build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "Build completed successfully!" -ForegroundColor Green
Write-Host "Press any key to continue..." -ForegroundColor Cyan
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
