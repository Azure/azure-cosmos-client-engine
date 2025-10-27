param(
    [string]$CargoTomlPath,
    [switch]$IncludeBuildNumber
)

if (-not $CargoTomlPath) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $CargoTomlPath = Join-Path $RepoRoot "Cargo.toml"
}

# Determine the version
$CargoMetadata = cargo metadata --manifest-path $CargoTomlPath --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to get cargo metadata"
    exit 1
}

$Version = $CargoMetadata.packages | Where-Object { $_.name -eq "cosmoscx" } | Select-Object -ExpandProperty version

if ($IncludeBuildNumber) {
    $SequenceNumber = [System.Environment]::GetEnvironmentVariable("BUILD_BUILDID")
    if (!$SequenceNumber) {
        $SequenceNumber = "local.$([int](([DateTime]::UtcNow - [DateTime]::UnixEpoch).TotalSeconds))"
    }
    if ($Version -match "-") {
        $Version += ".build.$SequenceNumber"
    }
    else {
        $Version += "-build.$SequenceNumber"
    }
}

Write-Output $Version