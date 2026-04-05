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

# Initialize or update cargo-dist configuration and CI workflows
dist-init:
    dist init

# Preview what cargo-dist will build in CI
dist-plan:
    dist plan

# Build distributable artifacts for the current platform
dist-build:
    dist build

# Smoke test dist release workflow locally with act (optional)
act-dist-plan:
    act pull_request -W .github/workflows/release.yml -j plan -P ubuntu-22.04=catthehacker/ubuntu:act-22.04

# Preview unreleased changelog
changelog:
    git cliff --unreleased

# Generate full CHANGELOG.md
changelog-gen:
    git cliff -o CHANGELOG.md

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

# Bump version, tag, publish all crates to crates.io (e.g. just release patch)
release LEVEL:
    cargo release {{LEVEL}} --execute

# Dry-run release (no commits, tags, or publishing)
release-dry-run LEVEL="patch":
    cargo release {{LEVEL}}

# Clean everything
clean:
    cargo clean
