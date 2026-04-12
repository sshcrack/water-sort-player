# Exit on error
$ErrorActionPreference = "Stop"

$SCRCPY_VERSION = "3.3.4"

# Create directories if they don't exist
if (-not (Test-Path "target/debug")) {
    New-Item -ItemType Directory -Path "target/debug" -Force | Out-Null
}
if (-not (Test-Path "target/release")) {
    New-Item -ItemType Directory -Path "target/release" -Force | Out-Null
}

# Download scrcpy if not already present
$scrcpyFile = "target/scrcpy-${SCRCPY_VERSION}.zip"
if (-not (Test-Path $scrcpyFile)) {
    Write-Host "Downloading scrcpy version ${SCRCPY_VERSION}..."
    $url = "https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/scrcpy-win64-v${SCRCPY_VERSION}.zip"
    Invoke-WebRequest -Uri $url -OutFile $scrcpyFile
}

function Expand-ZipStripFirstComponent {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ZipPath,

        [Parameter(Mandatory = $true)]
        [string]$DestinationPath
    )

    if (-not ('System.IO.Compression.ZipFile' -as [type])) {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
    }

    $zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path $ZipPath))
    try {
        foreach ($entry in $zip.Entries) {
            if ([string]::IsNullOrWhiteSpace($entry.FullName)) {
                continue
            }

            $parts = $entry.FullName -split '[\\/]' | Where-Object { $_ -ne '' }
            if ($parts.Count -le 1) {
                continue
            }

            $relativePath = ($parts | Select-Object -Skip 1) -join [System.IO.Path]::DirectorySeparatorChar
            if ([string]::IsNullOrWhiteSpace($relativePath)) {
                continue
            }

            $targetFile = Join-Path $DestinationPath $relativePath
            $targetDir = Split-Path $targetFile -Parent
            if (-not (Test-Path $targetDir)) {
                New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
            }

            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $targetFile, $true)
        }
    }
    finally {
        $zip.Dispose()
    }
}

# Extract zip files while stripping the leading archive folder
Expand-ZipStripFirstComponent -ZipPath $scrcpyFile -DestinationPath "target/debug"
Expand-ZipStripFirstComponent -ZipPath $scrcpyFile -DestinationPath "target/release"
