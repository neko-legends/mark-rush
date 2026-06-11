$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $Root

$CargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
if ($CargoCommand) {
    $Cargo = $CargoCommand.Source
} else {
    $Cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
}
if (-not (Test-Path -LiteralPath $Cargo)) {
    throw "Cargo was not found. Install Rust or add cargo to PATH."
}

$env:MARKRUSH_BUILD_STAMP = [DateTime]::UtcNow.ToString('yyyyMMddHHmmss')

& $Cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw "cargo build --release failed with exit code $LASTEXITCODE"
}

$ExePath = Join-Path $Root 'target\release\markrush.exe'
if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "Expected executable was not produced: $ExePath"
}

$Version = (Get-Item -LiteralPath $ExePath).VersionInfo.ProductVersion
if (-not $Version) {
    $Version = 'dev'
}

$ReleaseRoot = Join-Path $Root 'release'
$PortableDir = Join-Path $ReleaseRoot 'portable'
if (Test-Path -LiteralPath $PortableDir) {
    Remove-Item -LiteralPath $PortableDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null

$PackagePath = Join-Path $PortableDir 'markrush.exe'
Copy-Item -LiteralPath $ExePath -Destination $PackagePath -Force

$Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $PackagePath).Hash
$HashPath = "$PackagePath.sha256.txt"
Set-Content -LiteralPath $HashPath -Value "$Hash  $(Split-Path -Leaf $PackagePath)"

Write-Output "Build version: $Version"
Write-Output "Portable EXE: $PackagePath"
Write-Output "SHA256: $Hash"
