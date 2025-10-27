param(
    [string]$NewVersion,
    [string]$CargoTomlPath
)

if (-not $CargoTomlPath) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $CargoTomlPath = Join-Path $RepoRoot "Cargo.toml"
}

if (-not $NewVersion) {
    $CargoMetadata = cargo metadata --manifest-file $CargoTomlPath --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to get cargo metadata"
        exit 1
    }
    $CosmosCxPackage = $CargoMetadata.packages | Where-Object { $_.name -eq "cosmoscx" }
    $ExistingVersion = $CosmosCxPackage.version
    $Parts = $ExistingVersion -split "\."
    $Parts[2] = [int]$Parts[2] + 1
    $NewVersion = $Parts -join "."
    Write-Host "No version specified. Bumping patch version from $ExistingVersion to $NewVersion"
}

Write-Host "Updating version in $CargoTomlPath to $NewVersion"
(Get-Content $CargoTomlPath) -replace '^version = "\d+\.\d+\.\d+"$', "version = `"$NewVersion`"" | Set-Content $CargoTomlPath
