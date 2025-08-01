param(
    [string]$Prefix,
    [string]$Includedir
)

$PkgConfigContent = @"
prefix=${Prefix}
libdir=$${prefix}/lib
includedir=${Includedir}

Name: cosmoscx
Description: Azure Cosmos Client Engine
Version: 0.1.0
Cflags: -I$${includedir}
Libs: -L$${libdir} -lcosmoscx
"@
Set-Content -Path "${Prefix}/cosmoscx.pc" -Value $PkgConfigContent