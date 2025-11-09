# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

uptix is a Rust tool for pinning and updating external dependencies in NixOS configurations. It scans Nix files for `uptix.*` function calls, fetches the latest versions/digests for dependencies, and maintains an `uptix.lock` file for deterministic builds.

## Key Commands

### Usage
```bash
# Update all dependencies
uptix update

# Update a single dependency - Docker images
uptix update --dependency "postgres:15"
uptix update --dependency "homeassistant/home-assistant:stable"

# Update a single dependency - GitHub releases (matches uptix.githubRelease)
uptix update --dependency "owner/repo"
uptix update --dependency "luizribeiro/hello-world-rs"

# Update a single dependency - GitHub branches (matches uptix.githubBranch)
uptix update --dependency "owner/repo:branch"
uptix update --dependency "luizribeiro/hello-world-rs:main"

# You can also use the internal key format if needed
uptix update --dependency '$GITHUB_RELEASE$:owner/repo$'
uptix update --dependency '$GITHUB_BRANCH$:owner/repo:branch$'

# List all dependencies
uptix list

# Show detailed information about a dependency
uptix show "postgres:15"
uptix show "owner/repo"
uptix show "owner/repo:branch"

# Initialize an empty lock file
uptix init

# Use a custom lock file path (works with all commands)
# This allows running uptix from outside the project directory
uptix --lock-file /path/to/project/uptix.lock list
uptix --lock-file /path/to/project/uptix.lock update
uptix --lock-file /path/to/project/uptix.lock show "postgres:15"
uptix --lock-file /tmp/custom.lock init
```

### Docker Hub Authentication

uptix supports Docker Hub authentication to avoid rate limiting (100 pulls/6hrs anonymous vs 200+ authenticated):

**Option 1: Environment Variables (recommended for CI)**
```bash
export DOCKERHUB_USERNAME=your_username
export DOCKERHUB_TOKEN=your_personal_access_token
uptix update
```

**Option 2: ~/.docker/config.json (for local development)**
```bash
# Login with Docker CLI
docker login

# uptix will automatically use your credentials
uptix update
```

**For GitHub Actions:**
Add secrets `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` to your repository, then:
```yaml
- name: Update dependencies
  env:
    DOCKERHUB_USERNAME: ${{ secrets.DOCKERHUB_USERNAME }}
    DOCKERHUB_TOKEN: ${{ secrets.DOCKERHUB_TOKEN }}
  run: uptix update
```

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