# Build and package Controller Tray

$ErrorActionPreference = "Stop"

# Build release
Write-Host "Building release..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { exit 1 }

# Create distribution folder
$distDir = "dist\ControllerTray"
if (Test-Path $distDir) { Remove-Item -Recurse -Force $distDir }
New-Item -ItemType Directory -Path $distDir -Force | Out-Null
New-Item -ItemType Directory -Path "$distDir\assets" -Force | Out-Null

# Copy files
Write-Host "Packaging..." -ForegroundColor Cyan
Copy-Item "target\release\controller_app.exe" "$distDir\ControllerTray.exe"
Copy-Item "assets\game-controller.png" "$distDir\assets\"

# Create zip
$zipPath = "dist\ControllerTray.zip"
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
Compress-Archive -Path $distDir -DestinationPath $zipPath

Write-Host "Done! Package created at: $zipPath" -ForegroundColor Green
Write-Host "Contents:" -ForegroundColor Yellow
Get-ChildItem -Recurse $distDir | ForEach-Object { Write-Host "  $($_.FullName.Replace((Get-Location).Path + '\dist\', ''))" }
