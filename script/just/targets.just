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

target_env := if build_target =~ ".*-gnu" {
    "gnu"
} else if build_target =~ ".*-msvc" {
    "msvc"
} else if build_target =~ ".*-musl" {
    "musl"
} else if build_target =~ ".*-apple-darwin" {
    "apple"
} else {
    error("Unrecognized build target: " + build_target)
}

host_env := if host_target =~ ".*-gnu" {
    "gnu"
} else if host_target =~ ".*-msvc" {
    "msvc"
} else if host_target =~ ".*-musl" {
    "musl"
} else if host_target =~ ".*-apple-darwin" {
    "apple"
} else {
    error("Unrecognized host target: " + host_target)
}

build_static_lib := "true"
build_shared_lib := if target_env == "musl" {
    "false"
} else {
    "true"
}

# Detect some properties about the target
shared_lib_prefix := if target_os == "windows" { "" } else { "lib" }
shared_lib_extension := if target_os == "windows" { ".dll" } else if target_os == "macos" { ".dylib" } else { ".so" }
static_lib_prefix := if target_os == "windows" { 
    if target_env == "gnu" {
        "lib"
    } else {
        "" 
    }
} else { 
    "lib" 
}
static_lib_extension := if target_os == "windows" {
    if target_env == "gnu" {
        ".a"
    } else {
        ".lib"
    }
} else { 
    ".a" 
}

import_lib_prefix := if target_env == "msvc" {
    ""
} else {
    "lib"
}

import_lib_extension := if target_os == "windows" {
    if target_env == "gnu" {
        ".dll.a"
    } else {
        ".dll.lib"
    }
} else {
    ""
}

# Computes the cross-compiling linker to be used, if we're cross-compiling.
target_cross_linker := if target_os == "linux" {
    if target_env == "gnu" {
        target_arch + "-linux-gnu-gcc"
    } else {
        ""
    }
} else {
    ""
}

# Determines if we're cross-compiling, and activates the linker if we are.
linker := if host_target != build_target {
    target_cross_linker
} else {
    ""
}