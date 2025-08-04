#!/usr/bin/env pwsh
$RepoRoot = Split-Path $PSScriptRoot -Parent
$CurrentHeaderPath = Resolve-Path "${RepoRoot}/include/cosmoscx.h"
$GoHeaderPath = Resolve-Path "${RepoRoot}/go/azcosmoscx/include/cosmoscx.h"

$TempFile = (New-TemporaryFile).FullName
try {
    just header_path="$TempFile" _generate_headers

    Write-Host "Comparing headers..."
    diff $CurrentHeaderPath $TempFile
    if ($?) {
        Write-Host "Header file '$CurrentHeaderPath' is up-to-date"
    }
    else {
        Write-Host "Header file '$CurrentHeaderPath' is out-of-date. Run 'just headers' to update."
        exit 1
    }

    diff $GoHeaderPath $TempFile
    if ($?) {
        Write-Host "Header file '$GoHeaderPath' is up-to-date"
    }
    else {
        Write-Host "Header file '$GoHeaderPath' is out-of-date. Run 'just headers' to update."
        exit 1
    }
}
finally {
    Remove-Item -Path $TempFile -ErrorAction SilentlyContinue
}