#!/usr/bin/env bash
# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

prefix="${1}"
includedir="${2}"

cat <<EOF > "${prefix}/cosmoscx.pc"
prefix=${prefix}
libdir=\${prefix}/lib
includedir=${includedir}

Name: cosmoscx
Description: Azure Cosmos Client Engine
Version: 0.1.0
Cflags: -I\${includedir}
Libs: -L\${libdir} -lcosmoscx
EOF