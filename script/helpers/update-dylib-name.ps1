param(
    [string]$TargetOs,
    [string]$DylibPath
)

if ($env:CARGO_BUILD_TARGET.EndsWith("-musl")) {
    Write-Host "Skipping dylib name update for musl target"
}

if ($TargetOs -eq "macos") {
    # Update the dylib name to match the filename and use @rpath so it can be found immediately next to the executable (or anywhere else on an application's rpath)
    # In addition, add an rpath to this dylib pointing to the build directory so that local testing can find it easily.
    $BaseName = [System.IO.Path]::GetFileName($DylibPath)
    Write-Host "Updating dylib name for $DylibPath to @rpath/$BaseName"
    install_name_tool -id "@rpath/$BaseName" "$DylibPath"
}
else {
    Write-Host "No dylib name update needed for OS: $TargetOs"
}