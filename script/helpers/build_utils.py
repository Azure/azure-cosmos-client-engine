#!/usr/bin/env python3
# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

"""Cross-platform file operations for the build system."""

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


def get_platform():
    """Get normalized platform name."""
    system = platform.system().lower()
    if system == "darwin":
        return "macos"
    elif system == "windows":
        return "windows"
    else:
        return "linux"


def get_lib_extensions():
    """Get library file extensions for current platform."""
    plat = get_platform()
    if plat == "windows":
        return {"shared": ".dll", "static": ".lib"}
    elif plat == "macos":
        return {"shared": ".dylib", "static": ".a"}
    else:  # linux
        return {"shared": ".so", "static": ".a"}


def copy_artifacts(source_dir: Path, dest_dir: Path, cargo_build_target: str, configuration: str):
    """Copy build artifacts to the artifacts directory."""
    plat = get_platform()
    extensions = get_lib_extensions()

    # Ensure destination directory exists
    dest_lib_dir = dest_dir / "lib"
    dest_lib_dir.mkdir(parents=True, exist_ok=True)

    # Define source target directory
    source_target_dir = source_dir / cargo_build_target / configuration

    # Copy shared library
    if plat == "windows":
        shared_lib_name = "cosmoscx"
        static_lib_name = "cosmoscx"
        if cargo_build_target == "x86_64-pc-windows-gnu":
            shared_lib_filename = f"{shared_lib_name}.dll"
            static_lib_filename = f"lib{static_lib_name}.a"
        else:
            shared_lib_filename = f"{shared_lib_name}.dll"
            static_lib_filename = f"{static_lib_name}.lib"
    else:
        shared_lib_filename = f"libcosmoscx{extensions['shared']}"
        static_lib_filename = f"libcosmoscx{extensions['static']}"

    # Copy files if they exist
    shared_src = source_target_dir / shared_lib_filename
    static_src = source_target_dir / static_lib_filename

    if shared_src.exists():
        shutil.copy2(shared_src, dest_lib_dir / shared_lib_filename)
        print(f"Copied {shared_src} -> {dest_lib_dir / shared_lib_filename}")

        # Update dylib name on macOS
        if plat == "macos":
            update_dylib_name(dest_lib_dir / shared_lib_filename)

    if static_src.exists():
        shutil.copy2(static_src, dest_lib_dir / static_lib_filename)
        print(f"Copied {static_src} -> {dest_lib_dir / static_lib_filename}")


def update_dylib_name(dylib_path: Path):
    """Update the dylib name on macOS to use @rpath."""
    if get_platform() != "macos":
        return

    print(f"Updating dylib name for {dylib_path} to @rpath/{dylib_path.name}")
    try:
        subprocess.run([
            "install_name_tool", "-id", f"@rpath/{dylib_path.name}", str(
                dylib_path)
        ], check=True)
    except subprocess.CalledProcessError as e:
        print(f"Warning: Failed to update dylib name: {e}", file=sys.stderr)


def write_pkg_config(artifacts_dir: Path, include_dir: Path):
    """Write pkg-config file for the library."""
    pkg_config_content = f"""prefix={artifacts_dir}
libdir=${{prefix}}/lib
includedir={include_dir}

Name: cosmoscx
Description: Azure Cosmos Client Engine
Version: 0.1.0
Cflags: -I${{includedir}}
Libs: -L${{libdir}} -lcosmoscx
"""

    pkg_config_path = artifacts_dir / "cosmoscx.pc"
    with open(pkg_config_path, 'w') as f:
        f.write(pkg_config_content)
    print(f"Wrote pkg-config file to {pkg_config_path}")


def copy_vendor_lib(artifacts_dir: Path, root_dir: Path, cargo_build_target: str):
    """Copy library to vendor directory for Go builds."""
    plat = get_platform()
    vendor_dir = root_dir / "go" / "azcosmoscx" / \
        "libcosmoscx-vendor" / cargo_build_target

    # Ensure vendor directory exists
    vendor_dir.mkdir(parents=True, exist_ok=True)

    # Determine static library filename based on platform and target
    if plat == "windows":
        if cargo_build_target == "x86_64-pc-windows-gnu":
            static_lib_filename = "libcosmoscx.a"
        else:
            static_lib_filename = "cosmoscx.lib"
    else:
        static_lib_filename = "libcosmoscx.a"

    src = artifacts_dir / "lib" / static_lib_filename
    dst = vendor_dir / static_lib_filename

    if src.exists():
        shutil.copy2(src, dst)
        print(f"Copied {src} -> {dst}")
    else:
        print(f"Warning: {src} does not exist", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(
        description="Build system helper utilities")
    subparsers = parser.add_subparsers(
        dest="command", help="Available commands")

    # copy-artifacts command
    copy_parser = subparsers.add_parser(
        "copy-artifacts", help="Copy build artifacts")
    copy_parser.add_argument("source_dir", type=Path,
                             help="Source target directory")
    copy_parser.add_argument("dest_dir", type=Path,
                             help="Destination artifacts directory")
    copy_parser.add_argument("cargo_build_target", help="Cargo build target")
    copy_parser.add_argument("configuration", help="Build configuration")

    # write-pkg-config command
    pkg_parser = subparsers.add_parser(
        "write-pkg-config", help="Write pkg-config file")
    pkg_parser.add_argument("artifacts_dir", type=Path,
                            help="Artifacts directory")
    pkg_parser.add_argument("include_dir", type=Path, help="Include directory")

    # update-dylib-name command
    dylib_parser = subparsers.add_parser(
        "update-dylib-name", help="Update dylib name on macOS")
    dylib_parser.add_argument("dylib_path", type=Path,
                              help="Path to dylib file")

    # copy-vendor-lib command
    vendor_parser = subparsers.add_parser(
        "copy-vendor-lib", help="Copy library to vendor directory for Go builds")
    vendor_parser.add_argument(
        "artifacts_dir", type=Path, help="Artifacts directory")
    vendor_parser.add_argument(
        "root_dir", type=Path, help="Root directory of the Go module")
    vendor_parser.add_argument("cargo_build_target", help="Cargo build target")

    args = parser.parse_args()

    if args.command == "copy-artifacts":
        copy_artifacts(args.source_dir, args.dest_dir,
                       args.cargo_build_target, args.configuration)
    elif args.command == "write-pkg-config":
        write_pkg_config(args.artifacts_dir, args.include_dir)
    elif args.command == "update-dylib-name":
        update_dylib_name(args.dylib_path)
    elif args.command == "copy-vendor-lib":
        copy_vendor_lib(args.artifacts_dir, args.root_dir,
                        args.cargo_build_target)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
