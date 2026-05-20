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

$DotnetCommand = Get-Command dotnet -ErrorAction SilentlyContinue
if (-not $DotnetCommand) {
    throw ".NET SDK was not found. Install the .NET SDK to build the installer."
}
$Dotnet = $DotnetCommand.Source

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

$PackageVersion = $Version -replace '\+', '-'
$PackageName = "MarkRush-$PackageVersion-windows-x64-setup"
$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) "$PackageName-payload-$([Guid]::NewGuid().ToString('N'))"
$PublishDir = Join-Path ([System.IO.Path]::GetTempPath()) "$PackageName-publish-$([Guid]::NewGuid().ToString('N'))"
$PackagePath = Join-Path $DistDir "$PackageName.exe"

try {
    New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
    New-Item -ItemType Directory -Force -Path $PublishDir | Out-Null

    Copy-Item -LiteralPath $ExePath -Destination (Join-Path $StageDir 'markrush.exe')
    Copy-Item -LiteralPath (Join-Path $Root 'README.md') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'LICENSE') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\README.md') -Destination (Join-Path $StageDir 'font-licenses-README.md')
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-Lora.txt') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-NotoSans.txt') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-NotoSerif.txt') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-Poppins.txt') -Destination $StageDir
    Copy-Item -LiteralPath (Join-Path $Root 'assets\fonts\OFL-RobotoMono.txt') -Destination $StageDir

    if (Test-Path -LiteralPath $PackagePath) {
        Remove-Item -LiteralPath $PackagePath -Force
    }

    $InstallerProject = Join-Path $Root 'installer\MarkRushInstaller\MarkRushInstaller.csproj'
    & $Dotnet publish $InstallerProject `
        --configuration Release `
        --runtime win-x64 `
        --self-contained true `
        --output $PublishDir `
        -p:PayloadDir="$StageDir" `
        -p:MarkRushVersion="$Version" `
        -p:PublishSingleFile=true `
        -p:EnableCompressionInSingleFile=true
    if ($LASTEXITCODE -ne 0) {
        throw "dotnet publish failed with exit code $LASTEXITCODE"
    }

    $InstallerExe = Join-Path $PublishDir 'MarkRushInstaller.exe'
    if (-not (Test-Path -LiteralPath $InstallerExe)) {
        throw "Expected installer executable was not produced: $InstallerExe"
    }

    Copy-Item -LiteralPath $InstallerExe -Destination $PackagePath -Force
}
finally {
    foreach ($Path in @($StageDir, $PublishDir)) {
        if (Test-Path -LiteralPath $Path) {
            $ResolvedPath = (Resolve-Path -LiteralPath $Path).Path
            $TempRoot = [System.IO.Path]::GetTempPath()
            if ($ResolvedPath.StartsWith($TempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                Remove-Item -LiteralPath $ResolvedPath -Recurse -Force
            }
        }
    }
}

$Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $PackagePath).Hash
$HashPath = "$PackagePath.sha256.txt"
Set-Content -LiteralPath $HashPath -Value "$Hash  $(Split-Path -Leaf $PackagePath)"

Write-Output "Build version: $Version"
Write-Output "Installer: $PackagePath"
Write-Output "SHA256: $Hash"
