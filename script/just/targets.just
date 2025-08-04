host_target := `((rustc -vV | Select-String "host: ") -split ':')[1].Trim()`
build_target := env("CARGO_BUILD_TARGET", host_target)

# Decode the target
target_os := if build_target =~ ".*-unknown-linux-.*" {
    "linux"
} else if build_target =~ ".*-pc-windows-.*" {
    "windows"
} else if build_target =~ ".*-apple-darwin" {
    "macos"
} else {
    error("Unrecognized target: " + build_target)
}

target_arch := if build_target =~ "x86_64-.*" {
    "x86_64"
} else if build_target =~ "aarch64-.*" {
    "aarch64"
} else {
    error("Unrecognized target: " + build_target)
}

target_toolchain := if build_target =~ ".*-gnu" {
    "gnu"
} else if build_target =~ ".*-msvc" {
    "msvc"
} else if build_target =~ ".*-apple-darwin" {
    "apple"
} else {
    error("Unrecognized target: " + build_target)
}

# Detect some properties about the target
shared_lib_prefix := if target_os == "windows" { "" } else { "lib" }
shared_lib_extension := if target_os == "windows" { ".dll" } else if target_os == "macos" { ".dylib" } else { ".so" }
static_lib_prefix := if target_os == "windows" { 
    if target_toolchain == "gnu" {
        "lib"
    } else {
        "" 
    }
} else { 
    "lib" 
}
static_lib_extension := if target_os == "windows" {
    if target_toolchain == "gnu" {
        ".a"
    } else {
        ".lib"
    }
} else { 
    ".a" 
}

import_lib_prefix := if target_toolchain == "msvc" {
    ""
} else {
    "lib"
}

import_lib_extension := if target_os == "windows" {
    if target_toolchain == "gnu" {
        ".dll.a"
    } else {
        ".dll.lib"
    }
} else {
    ""
}