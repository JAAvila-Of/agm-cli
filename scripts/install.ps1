# AGM CLI installer for Windows
# Usage:
#   irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
#   $env:AGM_VERSION = 'v1.2.3'; irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
#   $env:AGM_INSTALL_DIR = 'C:\tools\agm'; irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo       = "JAAvila-Of/agm-cli"
$BinName    = "agm.exe"
$InstallDir = if ($env:AGM_INSTALL_DIR) {
    $env:AGM_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "Programs\agm\bin"
}

# --- Detect arch ----------------------------------------------------------
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" {
        Write-Warning "aarch64-pc-windows-msvc is not currently published. Aborting."
        Write-Warning "Install via: cargo install agm-cli"
        exit 1
    }
    default {
        Write-Error "Unsupported Windows architecture: $Arch"
        exit 1
    }
}

# --- Resolve version ------------------------------------------------------
if ($env:AGM_VERSION) {
    $VersionTag = $env:AGM_VERSION
} else {
    Write-Host "Fetching latest release..."
    $Release    = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $VersionTag = $Release.tag_name
}

if (-not $VersionTag) {
    Write-Error "Could not determine version to install"
    exit 1
}

# --- Download + verify ----------------------------------------------------
$Archive = "agm-$VersionTag-$Target.zip"
$Url     = "https://github.com/$Repo/releases/download/$VersionTag/$Archive"
$ShaUrl  = "$Url.sha256"

Write-Host "Downloading $Archive..."
$TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "agm-install-$(Get-Random)")

try {
    $ArchivePath = Join-Path $TmpDir $Archive
    $ShaPath     = "$ArchivePath.sha256"

    Invoke-WebRequest -Uri $Url    -OutFile $ArchivePath
    Invoke-WebRequest -Uri $ShaUrl -OutFile $ShaPath

    Write-Host "Verifying checksum..."
    $Expected = (Get-Content $ShaPath).Split()[0].ToLower()
    $Actual   = (Get-FileHash $ArchivePath -Algorithm SHA256).Hash.ToLower()
    if ($Expected -ne $Actual) {
        Write-Error "Checksum mismatch: expected $Expected, got $Actual"
        exit 1
    }

    Write-Host "Extracting..."
    Expand-Archive -Path $ArchivePath -DestinationPath $TmpDir -Force

    $StageDir = Join-Path $TmpDir "agm-$VersionTag-$Target"

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Write-Host "Installing to $InstallDir..."
    Copy-Item (Join-Path $StageDir $BinName) (Join-Path $InstallDir $BinName) -Force

    Write-Host ""
    Write-Host "Installed agm $VersionTag to $InstallDir\$BinName"

    # --- PATH hint ------------------------------------------------------
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "Note: $InstallDir is not in your PATH."
        Write-Host "Add it by running (one-time):"
        Write-Host ""
        Write-Host "    [Environment]::SetEnvironmentVariable('PATH', `"`$env:PATH;$InstallDir`", 'User')"
        Write-Host ""
        Write-Host "Then restart your terminal."
    } else {
        Write-Host "Run: agm --version"
    }
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
