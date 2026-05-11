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

$DistDir = Join-Path $Root 'dist'
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

$PackageName = "MarkRush-$Version-windows-x64-portable"
$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) "$PackageName-$([Guid]::NewGuid().ToString('N'))"
$PackagePath = Join-Path $DistDir "$PackageName.zip"

try {
    New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
    Copy-Item -LiteralPath $ExePath -Destination (Join-Path $StageDir 'markrush.exe')
    Copy-Item -LiteralPath (Join-Path $Root 'README.md') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'LICENSE') -Destination $StageDir

    $FontLicenseDir = Join-Path $StageDir 'font-licenses'
    New-Item -ItemType Directory -Force -Path $FontLicenseDir | Out-Null
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\README.md') -Destination $FontLicenseDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-Lora.txt') -Destination $FontLicenseDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-NotoSans.txt') -Destination $FontLicenseDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-NotoSerif.txt') -Destination $FontLicenseDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-Poppins.txt') -Destination $FontLicenseDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-RobotoMono.txt') -Destination $FontLicenseDir

    if (Test-Path -LiteralPath $PackagePath) {
        Remove-Item -LiteralPath $PackagePath -Force
    }
    Compress-Archive -Path (Join-Path $StageDir '*') -DestinationPath $PackagePath -Force
}
finally {
    if (Test-Path -LiteralPath $StageDir) {
        $ResolvedStage = (Resolve-Path -LiteralPath $StageDir).Path
        $TempRoot = [System.IO.Path]::GetTempPath()
        if ($ResolvedStage.StartsWith($TempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $ResolvedStage -Recurse -Force
        }
    }
}

$Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $PackagePath).Hash
$HashPath = "$PackagePath.sha256.txt"
Set-Content -LiteralPath $HashPath -Value "$Hash  $(Split-Path -Leaf $PackagePath)"

Write-Output "Build version: $Version"
Write-Output "Package: $PackagePath"
Write-Output "SHA256: $Hash"
