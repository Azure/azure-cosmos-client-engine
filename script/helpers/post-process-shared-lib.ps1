# Post-process the shared library
param(
    [string]$TargetOs,
    [string]$TargetDir,
    [string]$SharedLibFilename,
    [string]$ArtifactsDir,
    [string]$ImportLibFileName
)

if ($ImportLibFileName) {
    Copy-Item $TargetDir/$ImportLibFileName $ArtifactsDir/lib/$ImportLibFileName
}

& script/helpers/update-dylib-name.ps1 -TargetOS $TargetOs -DylibPath $TargetDir/$SharedLibFilename
& script/helpers/write-pkg-config.ps1 -Prefix $ArtifactsDir -Includedir $ArtifactsDir/include