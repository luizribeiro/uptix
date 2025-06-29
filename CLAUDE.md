# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

uptix is a Rust tool for pinning and updating external dependencies in NixOS configurations. It scans Nix files for `uptix.*` function calls, fetches the latest versions/digests for dependencies, and maintains an `uptix.lock` file for deterministic builds.

## Key Commands

### Development
```bash
# Build the project
cargo build

# Run tests
cargo test

# Run a specific test
cargo test test_name

# Format code
cargo fmt

# Check code without building
cargo check

# Run with debug output
RUST_LOG=debug cargo run -- [args]
```

### Testing & CI
```bash
# Run full CI checks locally
cargo fmt --all -- --check
cargo test
cargo build

# Test against example configuration
cd example && nix build .#nixosConfigurations.vm.config.system.build.toplevel
```

### Nix Commands
```bash
# Build via Nix
nix build .#packages.x86_64-linux.uptix

# Enter development shell
nix develop

# Test the NixOS module integration
cd example && nix build
```

## Architecture

### Core Flow
1. **main.rs**: Entry point that orchestrates the entire process
   - Discovers Nix files in the project
   - Parses them to find `uptix.*` function calls
   - Delegates to dependency handlers
   - Writes the lock file

2. **deps/**: Dependency type implementations
   - **docker.rs**: Handles Docker image dependencies, including authentication and registry API calls
   - **github/branch.rs**: Fetches latest commit SHA for GitHub branches
   - **github/release.rs**: Fetches latest release information from GitHub

3. **util.rs**: Nix file parsing utilities using the `rnix` crate

4. **modules/default.nix**: NixOS module that reads `uptix.lock` and provides the `uptix.*` functions

### Key Patterns

#### Error Handling
- Uses `miette` for rich error reporting
- All errors implement proper error chains
- Docker registry errors include authentication context

#### Async Operations
- Uses `tokio` runtime for concurrent dependency fetching
- All network operations are async
- Dependencies are fetched in parallel for performance

#### Testing
- Unit tests are embedded in source files using `#[cfg(test)]`
- Test utilities in `deps/test_util.rs` provide mock HTTP responses
- Integration testing via example configuration in CI

### Supported Dependencies

1. **Docker Images**: `uptix.dockerImage "image:tag"`
   - Supports Docker Hub (including official images like `postgres:15`)
   - Supports other registries via full URLs
   - Handles authentication via Docker config

2. **GitHub Branches**: `uptix.githubBranch { owner = "..."; repo = "..."; branch = "..."; }`
   - Tracks latest commit SHA

3. **GitHub Releases**: `uptix.githubRelease { owner = "..."; repo = "..."; }`
   - Tracks latest release tag and tarball URL

### Important Files

- **src/deps/mod.rs**: Defines the `Dependency` trait that all dependency types must implement
- **src/error.rs**: Central error type definitions
- **modules/default.nix**: The Nix module interface that users interact with
- **example/**: Working examples of all dependency types