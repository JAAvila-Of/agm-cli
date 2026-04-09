# AGM CLI installer for Windows
# Usage: irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "JAAvila-Of/agm-cli"
$BinName = "agm.exe"
$InstallDir = if ($env:AGM_INSTALL_DIR) { $env:AGM_INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }

# Detect architecture
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" { $Target = "aarch64-pc-windows-msvc" }
    default {
        Write-Error "Unsupported architecture: $Arch"
        exit 1
    }
}

# Get latest release
Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name.TrimStart("v")
$Archive = "agm-v$Version-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$($Release.tag_name)/$Archive"

Write-Host "Downloading agm $Version for $Target..."
$TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "agm-install-$(Get-Random)")

try {
    $ArchivePath = Join-Path $TmpDir $Archive
    Invoke-WebRequest -Uri $Url -OutFile $ArchivePath

    Write-Host "Extracting..."
    Expand-Archive -Path $ArchivePath -DestinationPath $TmpDir -Force

    Write-Host "Installing to $InstallDir..."
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Copy-Item (Join-Path $TmpDir $BinName) (Join-Path $InstallDir $BinName) -Force

    Write-Host ""
    Write-Host "agm $Version installed to $InstallDir\$BinName"

    # Check if install dir is in PATH
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "Note: $InstallDir is not in your PATH."
        $AddToPath = Read-Host "Add it now? (y/N)"
        if ($AddToPath -eq "y" -or $AddToPath -eq "Y") {
            [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstallDir", "User")
            $env:PATH = "$env:PATH;$InstallDir"
            Write-Host "Added to PATH. Restart your terminal for changes to take effect."
        } else {
            Write-Host "You can add it manually later:"
            Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"`$env:PATH;$InstallDir`", 'User')"
        }
    }
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
