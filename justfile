# List available recipes
default:
    @just --list

run:
    cargo run -p wisp -- -a 'cargo run -p aether-agent-cli acp'

# Build the workspace
build:
    cargo build

# Check the workspace
check:
    cargo check

# Run tests with nextest
test *ARGS:
    cargo nextest run {{ARGS}}

# Run tests for a specific package
test-pkg PKG *ARGS:
    cargo nextest run -p {{PKG}} {{ARGS}}

# Format code
fmt:
    cargo fmt

# Check formatting without modifying
fmt-check:
    cargo fmt --check

# Run clippy
lint:
    cargo clippy --workspace

# Format + lint
check-all: fmt-check lint

# Update packages/llm/models.json from models.dev
update-models:
    ./packages/llm/scripts/fetch-models.sh

# Sweep build artifacts older than N days (default: 7)
sweep DAYS="7":
    cargo sweep --time {{DAYS}}

# Sweep artifacts not used by the current toolchain
sweep-installed:
    cargo sweep --installed

# Build the workspace sandbox image
build-sandbox TAG="aether-sandbox:latest":
    docker build -t {{TAG}} -f Dockerfile.sandbox .

# Run wisp + aether agent inside the sandbox
run-sandbox:
    cargo run -p wisp -- -a 'cargo run -p aether-agent-cli -- --sandbox-image aether-sandbox:latest acp'

# Install aether-cli and wisp binaries locally
install:
    cargo install --path packages/aether-cli --force
    cargo install --path packages/wisp --force

# Publish all crates to crates.io in dependency order
release:
    #!/usr/bin/env bash
    set -euo pipefail
    # Leaf crates (no internal deps)
    cargo publish -p aether-utils
    cargo publish -p aether-llm-codegen
    cargo publish -p aether-tui
    cargo publish -p aether-lspd
    # Mid-tier
    cargo publish -p aether-llm
    cargo publish -p aether-mcp-utils
    cargo publish -p aether-acp-utils
    # Core
    cargo publish -p aether-agent-core
    # Upper-tier
    cargo publish -p aether-project
    cargo publish -p wisp
    cargo publish -p aether-mcp-servers
    # Top-level
    cargo publish -p aether-agent-cli

# Dry-run publish all crates in dependency order
release-dry-run:
    #!/usr/bin/env bash
    set -euo pipefail
    for pkg in aether-utils aether-llm-codegen aether-tui aether-lspd \
               aether-llm aether-mcp-utils aether-acp-utils \
               aether-agent-core aether-project wisp aether-mcp-servers \
               aether-agent-cli; do
        echo "--- dry-run: $pkg ---"
        cargo publish -p "$pkg" --dry-run --allow-dirty
    done

# Clean everything
clean:
    cargo clean
