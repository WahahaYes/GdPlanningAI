# GdPlanningAI Rust Build System

## Overview

This document describes the build system for the GdPlanningAI Rust extension. The build system automates the process of compiling the Rust code and copying the resulting libraries to the appropriate directories for Godot to load.

## Location

The build system is located in the `addons/GdPlanningAI/rust/` directory alongside the Rust source code:

```
addons/GdPlanningAI/rust/
├── Makefile              # Main build system
├── BUILD_SYSTEM.md       # This documentation
├── Cargo.toml           # Rust project configuration
├── src/                 # Rust source files
├── target/              # Cargo build output
└── tests/               # Rust test files
```

## Prerequisites

### Required Tools
- **Rust toolchain** (from https://rustup.rs/)
- **Make** (build tool)
- **POSIX commands** (rm, cp, mkdir) - available via:
  - Git Bash (recommended for Windows)
  - WSL (Windows Subsystem for Linux)
  - GNU Core Utils
  - Native Linux/macOS terminals
- **Docker or Podman** (required for cross-compilation with cross)
  - Docker Desktop (Windows/macOS)
  - Docker Engine (Linux)
  - Podman (alternative to Docker)

### Setup Commands

```bash
# Navigate to rust directory
cd addons/GdPlanningAI/rust

# Install cross for cross-compilation (requires Docker/Podman)
make setup-cross

# Show all available commands
make help
```

### Docker/Podman Requirements

**Before building for other platforms, ensure your container engine is running:**

- **Windows/macOS**: Start Docker Desktop
- **Linux**: Run `sudo systemctl start docker` or use Podman

The `cross` tool uses containers to provide pre-configured cross-compilation environments.

### Cross Configuration

The `Cross.toml` file configures custom Docker images when needed for cross-compilation:

Right now there is a `GetHostNameW` undefined reference error that occurs with the standard cross image for Windows, so we use a custom image.

## Usage

To use the build system, navigate to the rust directory:

```bash
cd addons/GdPlanningAI/rust
make help          # Show available commands
make build-debug   # Build debug binary
```

## Architecture

```
GdPlanningAI/
├── addons/GdPlanningAI/
│   ├── rust/               # Rust source code and build system
│   │   ├── Makefile        # Build system
│   │   ├── BUILD_SYSTEM.md # Documentation
│   │   ├── Cargo.toml      # Rust project configuration
│   │   ├── src/            # Rust source files
│   │   ├── target/         # Cargo build output
│   │   └── tests/          # Rust test files
│   ├── bin/                # Final binary destinations
│   │   ├── linux/          # Linux .so files
│   │   ├── windows/        # Windows .dll files
│   │   └── macos/          # macOS .dylib files
│   └── gdplanningai.gdextension  # GDExtension configuration
└── scripts/
    └── gdpai_rust_bridge.gd  # GDScript-Rust bridge
```

## Build Targets

### Platform-Specific Builds

```bash
# Build for current platform (debug)
make build-debug

# Build for current platform (release)
make build-release

# Build for all supported platforms
make build-all

# Build for specific platforms
make build-linux
make build-windows

# Clean all binary directories
make clean-binaries
```

### Development Tools

```bash
# Run tests
make test              # Run unit tests

# Code quality
make format            # Format code with rustfmt
make lint              # Run clippy linter
make lint-fix          # Auto-fix clippy issues

# Documentation
make doc               # Generate and open docs

# Cleanup
make clean             # Clean build artifacts
```

### Setup

```bash
# Install cross for cross-compilation (requires Docker/Podman)
make setup-cross

# Show help
make help
```

## Cross-Platform Support

The build system supports cross-compilation for Linux and Windows platforms:

### Linux (x86_64)
- **Target**: `x86_64-unknown-linux-gnu`
- **Output**: `bin/linux/libgdplanningai_rust.so`
- **Requirements**: Docker/Podman for cross-compilation

### Windows (x86_64)
- **Target**: `x86_64-pc-windows-gnu`
- **Output**: `bin/windows/gdplanningai_rust.dll`
- **Requirements**: Docker/Podman for cross-compilation
- **Note**: Uses custom cross image with updated mingw-w64 to fix GetHostNameW linking

### macOS Support
macOS users need to build binaries natively on macOS machines:

**Prerequisites:**
- macOS with Xcode Command Line Tools installed
- Rust toolchain with macOS targets

**Build Instructions:**
```bash
# On macOS machine:
cd addons/GdPlanningAI/rust

# Install macOS targets (if not already installed)
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Build for Intel Mac
cargo build --release --target x86_64-apple-darwin

# Build for Apple Silicon Mac  
cargo build --release --target aarch64-apple-darwin

# Create universal binary (optional)
lipo -create \
    target/x86_64-apple-darwin/release/libgdplanningai_rust.dylib \
    target/aarch64-apple-darwin/release/libgdplanningai_rust.dylib \
    -output ../bin/macos/libgdplanningai_rust.dylib

# Or copy individual binaries
mkdir -p ../bin/macos
cp target/x86_64-apple-darwin/release/libgdplanningai_rust.dylib ../bin/macos/
```

**TODO**: Set up cross-compilation for macOS using the custom Docker image from [cross-toolchains](https://github.com/cross-rs/cross-toolchains). This would enable automated macOS builds from any platform.

## GDExtension Configuration

The `gdplanningai.gdextension` file is configured to load the appropriate library based on platform and build mode:

```ini
[libraries]
linux.debug.x86_64 = "res://addons/GdPlanningAI/bin/linux/libgdplanningai_rust.so"
windows.debug.x86_64 = "res://addons/GdPlanningAI/bin/windows/gdplanningai_rust.dll"
macos.debug = "res://addons/GdPlanningAI/bin/macos/libgdplanningai_rust.dylib"
```

## Development Workflow

### Initial Setup

1. Install Rust toolchain
2. Install Docker Desktop (Windows/macOS) or Docker/Podman (Linux)
3. Navigate to rust directory and install cross:
   ```bash
   cd addons/GdPlanningAI/rust
   make setup-cross
   ```

### Daily Development

1. Navigate to rust directory:
   ```bash
   cd addons/GdPlanningAI/rust
   ```
2. Make changes to Rust code
3. Build and test:
   ```bash
   make build-debug
   make test
   ```
4. Run in Godot to test integration

### Release Preparation

1. Run full test suite:
   ```bash
   make test
   make lint
   ```
2. Build release binaries:
   ```bash
   make build-all
   ```
3. Verify all platforms have binaries in `bin/` directories

## Troubleshooting

### Common Issues

1. **"make: command not found"**
   - Install Make (Windows: via Chocolatey, Linux: via package manager)

2. **"cargo: command not found"**
   - Install Rust from https://rustup.rs/

3. **Cross-compilation fails**
   - Ensure Docker/Podman is running
   - Run `make setup-cross` to install cross
   - Check container engine status

4. **POSIX commands not found**
   - Use Git Bash (recommended for Windows)
   - Install GNU Core Utils
   - Use WSL or native Unix terminal

5. **Docker/Podman not running**
   - Start Docker Desktop (Windows/macOS)
   - Run `sudo systemctl start docker` (Linux)
   - Use `docker ps` or `podman ps` to verify

6. **Binary not found in Godot**
   - Check that binaries exist in `bin/` directories
   - Verify `.gdextension` file paths are correct

### Debug Mode vs Release Mode

- **Debug**: Faster compilation, includes debug info, larger binaries
- **Release**: Optimized code, smaller binaries, better performance

Use debug mode for development and release mode for production/testing.

## Integration with Godot

The build system integrates seamlessly with Godot:

1. **Automatic Loading**: Godot automatically loads the GDExtension based on `.gdextension` configuration
2. **Hot Reload**: For debug builds, you can rebuild and restart Godot to pick up changes
3. **Platform Detection**: Godot automatically selects the correct library for the current platform

## Performance Considerations

- **Release builds** are significantly faster than debug builds
- **Universal macOS binaries** support both Intel and Apple Silicon
- **Cross-compilation** allows building all platforms from a single machine

## Future Enhancements

Potential improvements to the build system:

1. **CI/CD Integration**: GitHub Actions for automated builds
2. **Version Management**: Automatic version bumping
3. **Asset Bundling**: Include test scenes and examples
4. **Documentation Generation**: Auto-generate API docs
5. **Performance Benchmarking**: Automated performance testing
