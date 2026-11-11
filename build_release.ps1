$ErrorActionPreference = "Stop"

$root = $PSScriptRoot
Push-Location $root

try {
    Write-Host "==> 1. Running core tests..." -ForegroundColor Cyan
    cargo test
    if ($LASTEXITCODE -ne 0) { throw "Core cargo test failed" }

    Write-Host "==> 2. Building Kurowser release binary..." -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Core cargo build --release failed" }

    Copy-Item "target\release\kurowser.exe" "kurowser.exe" -Force

    Write-Host "==> 3. Running installer tests..." -ForegroundColor Cyan
    Push-Location "installer"
    cargo test
    if ($LASTEXITCODE -ne 0) { throw "Installer cargo test failed" }

    Write-Host "==> 4. Building KurowserSetup release binary..." -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Installer cargo build --release failed" }
    Pop-Location

    Copy-Item "installer\target\release\kurowser-setup.exe" "KurowserSetup.exe" -Force

    $appItem = Get-Item "kurowser.exe"
    $setupItem = Get-Item "KurowserSetup.exe"
    $appHash = (Get-FileHash "kurowser.exe" -Algorithm SHA256).Hash
    $setupHash = (Get-FileHash "KurowserSetup.exe" -Algorithm SHA256).Hash

    Write-Host ""
    Write-Host "Build completed cleanly:" -ForegroundColor Green
    Write-Host ("  kurowser.exe      : {0:N2} MiB ({1} bytes) SHA256={2}" -f ($appItem.Length / 1MB), $appItem.Length, $appHash)
    Write-Host ("  KurowserSetup.exe : {0:N2} MiB ({1} bytes) SHA256={2}" -f ($setupItem.Length / 1MB), $setupItem.Length, $setupHash)
}
finally {
    Pop-Location
}

