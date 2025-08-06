target_root := justfile_directory() / "target"
artifacts_root := justfile_directory() / "artifacts"
configuration := env("CONFIGURATION", "debug")
target_dir := target_root / build_target / configuration
artifacts_dir := artifacts_root / build_target / configuration
cargo_profile := if configuration == "debug" { "dev" } else { configuration }
lib_name := "cosmoscx"
header_name := lib_name + ".h"
header_path := justfile_directory() / "include" / header_name
go_library_mode := "static"

shared_lib_filename := shared_lib_prefix + lib_name + shared_lib_extension
static_lib_filename := static_lib_prefix + lib_name + static_lib_extension
import_lib_filename := if target_os == "windows" { import_lib_prefix + lib_name + import_lib_extension } else { "" }

export CARGO_BUILD_TARGET := build_target
export PKG_CONFIG_PATH := artifacts_dir

# We don't want to use the winapi crates' bundled import libraries, because we're building a library to be consumed in other non-Rust toolchains.
export WINAPI_NO_BUNDLED_LIBRARIES := "true"

go_tags := (if go_library_mode == "shared" {
  "dynamic,"
} else {
  ""
}) + (if configuration == "vendored" {
  ""
} else if configuration == "debug" {
  "debug,azcosmoscx_local,"
} else {
  "azcosmoscx_local,"
})

# Set library load paths so that we can run shared library tests
# This uses the host os, since if we're going to run tests, it's going to be on the host OS.
export PATH := if os() == "windows" {
  artifacts_dir / "lib" + ";" + env("PATH", "")
} else {
  env("PATH", "")
}

export DYLD_LIBRARY_PATH := if os() == "macos" {
  artifacts_dir / "lib" + ":" + env("DYLD_LIBRARY_PATH", "")
} else {
  env("DYLD_LIBRARY_PATH", "")
}

export LD_LIBRARY_PATH := if os() == "linux" {
  artifacts_dir / "lib" + ":" + env("LD_LIBRARY_PATH", "")
} else {
  env("LD_LIBRARY_PATH", "")
}

# "&&" doesn't work as you'd hope in Just, so we have to do nested ifs.
# See https://github.com/casey/just/issues/2411
export CGO_LDFLAGS := if target_os == "windows" {
  if go_library_mode == "shared" {
    "-L" +artifacts_dir / "lib"
  } else {
    ""
  }
} else {
  ""
}