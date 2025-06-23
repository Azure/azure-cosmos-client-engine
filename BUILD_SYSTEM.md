# Build System Migration Guide

This repository has been migrated from Make to [Just](https://github.com/casey/just) as the primary build system.

## Quick Start

### Install Just

```bash
# Install via cargo (recommended)
cargo install just

# Or via package managers:
# macOS: brew install just
# Windows: choco install just
# Many Linux distros also have packages
```

### Basic Usage

```bash
# Show all available recipes
just --list

# Build everything (default)
just

# Build specific components
just headers        # Generate C headers
just engine_rust    # Build Rust engine
just engine_c       # Build C API
just engine_python  # Build Python extension

# Run tests
just test           # All tests
just test_rust      # Rust tests only
just test_go        # Go tests only

# Clean builds
just clean          # Clean all
just superclean     # Delete everything
```

## What Changed

### Benefits of Just over Make

1. **Cross-platform**: Works identically on Windows, macOS, and Linux
2. **No POSIX dependencies**: No need for bash/sh on Windows
3. **Modern syntax**: More readable and maintainable
4. **Better string handling**: Native support for conditionals and variables
5. **Python integration**: Uses Python for cross-platform scripting

### File Changes

- `Makefile` → `justfile`
- Shell scripts → Python scripts in `script/helpers/build_utils.py`
- All original functionality preserved

### Environment Variables

The same environment variables work as before:

```bash
CONFIGURATION=release just engine    # Release build
LIBRARY_MODE=shared just engine_c    # Shared library
VENDORED=true just test_go          # Vendored builds
```

## Cross-Platform Scripting

Complex build operations that require cross-platform compatibility are handled by Python scripts in `script/helpers/build_utils.py`:

- `copy-artifacts`: Copy build artifacts to the correct locations
- `write-pkg-config`: Generate pkg-config files
- `update-dylib-name`: Update macOS dylib names
- `copy-vendor-lib`: Copy libraries to vendor directories for Go builds

The Python script automatically handles platform-specific logic for file extensions, paths, and operations.

## Migration Notes

- All original Make targets are available as Just recipes
- Environment variables and configurations work the same way
- The build process and output are identical
- CI/CD systems can use `just` instead of `make`

## Examples

```bash
# Debug build (default)
just

# Release build
CONFIGURATION=release just engine

# Clean and rebuild everything
just superclean && just

# Build and test Go bindings
just engine_c && just test_go

# Check current configuration
just config
```
