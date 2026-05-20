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

function Get-MakeNsis {
    $Command = Get-Command makensis.exe -ErrorAction SilentlyContinue
    if ($Command) {
        return $Command.Source
    }

    foreach ($Candidate in @(
        (Join-Path $env:ProgramFiles 'NSIS\makensis.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'),
        (Join-Path $PSScriptRoot 'target\nsis\nsis-3.11\makensis.exe')
    )) {
        if ($Candidate -and (Test-Path -LiteralPath $Candidate)) {
            return $Candidate
        }
    }

    $NsisRoot = Join-Path $PSScriptRoot 'target\nsis'
    $NsisZip = Join-Path $NsisRoot 'nsis-3.11.zip'
    $NsisUrl = 'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.11/nsis-3.11.zip'

    New-Item -ItemType Directory -Force -Path $NsisRoot | Out-Null
    $NeedsDownload = $true
    if (Test-Path -LiteralPath $NsisZip) {
        $ExistingZip = Get-Item -LiteralPath $NsisZip
        $NeedsDownload = $ExistingZip.Length -lt 1MB
    }
    if ($NeedsDownload) {
        Remove-Item -LiteralPath $NsisZip -Force -ErrorAction SilentlyContinue
        Write-Host "Downloading portable NSIS 3.11..."
        $Curl = Get-Command curl.exe -ErrorAction SilentlyContinue
        if ($Curl) {
            & $Curl.Source -L -o $NsisZip $NsisUrl
            if ($LASTEXITCODE -ne 0) {
                throw "curl failed to download NSIS with exit code $LASTEXITCODE"
            }
        } else {
            Invoke-WebRequest -Uri $NsisUrl -OutFile $NsisZip -MaximumRedirection 10
        }
    }

    Expand-Archive -LiteralPath $NsisZip -DestinationPath $NsisRoot -Force
    $DownloadedMakeNsis = Join-Path $NsisRoot 'nsis-3.11\makensis.exe'
    if (-not (Test-Path -LiteralPath $DownloadedMakeNsis)) {
        throw "makensis.exe was not found after downloading NSIS."
    }

    return $DownloadedMakeNsis
}

$MakeNsis = Get-MakeNsis

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
$NumericVersion = if ($Version -match '^v?(\d+)\.(\d+)\.(\d+)') {
    "$($Matches[1]).$($Matches[2]).$($Matches[3]).0"
} else {
    '0.0.0.0'
}
$PackageName = "MarkRush-$PackageVersion-windows-x64-setup"
$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) "$PackageName-payload-$([Guid]::NewGuid().ToString('N'))"
$PackagePath = Join-Path $DistDir "$PackageName.exe"

try {
    New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

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

    $InstallerScript = Join-Path $Root 'installer\MarkRush.nsi'
    & $MakeNsis `
        "/DAPP_VERSION=$Version" `
        "/DAPP_NUMERIC_VERSION=$NumericVersion" `
        "/DOUT_FILE=$PackagePath" `
        "/DPAYLOAD_DIR=$StageDir" `
        $InstallerScript
    if ($LASTEXITCODE -ne 0) {
        throw "makensis failed with exit code $LASTEXITCODE"
    }
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

if (-not (Test-Path -LiteralPath $PackagePath)) {
    throw "Expected installer was not produced: $PackagePath"
}

$Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $PackagePath).Hash
$HashPath = "$PackagePath.sha256.txt"
Set-Content -LiteralPath $HashPath -Value "$Hash  $(Split-Path -Leaf $PackagePath)"

Write-Output "Build version: $Version"
Write-Output "Installer: $PackagePath"
Write-Output "SHA256: $Hash"
