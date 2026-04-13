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
test *PKGS:
    cargo nextest run --all-features {{ if PKGS == "" { "--workspace" } else { PKGS } }}

# Check formatting
fmt-check *PKGS:
    cargo fmt --check {{ if PKGS == "" { "--all" } else { PKGS } }}

# Format code
fmt:
    cargo fmt --all

# Run clippy
lint *PKGS:
    cargo clippy --all-targets --all-features {{ if PKGS == "" { "--workspace" } else { PKGS } }} -- -D warnings

# Check documentation builds without warnings
doc-check *PKGS:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features {{ if PKGS == "" { "--workspace --examples" } else { PKGS } }}

# Run all CI checks
ci: fmt-check lint test doc-check

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

# Release only packages with changes since their last tag
release LEVEL:
    #!/usr/bin/env bash
    set -euo pipefail
    pkgs=""
    while IFS=$'\t' read -r name dir; do
        tag=$(git tag -l "${name}-v*" --sort=-v:refname | head -1)
        if [ -z "$tag" ] || [ -n "$(git log "${tag}..HEAD" -- "$dir")" ]; then
            pkgs="$pkgs -p $name"
        fi
    done < <(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | [.name, (.manifest_path | split("/") | .[:-1] | join("/"))] | @tsv')
    if [ -z "$pkgs" ]; then
        echo "No packages have changes to release"
        exit 0
    fi
    echo "Releasing:$pkgs"
    cargo release {{LEVEL}} $pkgs --execute

# Dry-run release (no commits, tags, or publishing)
release-dry-run LEVEL="patch":
    #!/usr/bin/env bash
    set -euo pipefail
    pkgs=""
    while IFS=$'\t' read -r name dir; do
        tag=$(git tag -l "${name}-v*" --sort=-v:refname | head -1)
        if [ -z "$tag" ] || [ -n "$(git log "${tag}..HEAD" -- "$dir")" ]; then
            pkgs="$pkgs -p $name"
        fi
    done < <(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | [.name, (.manifest_path | split("/") | .[:-1] | join("/"))] | @tsv')
    if [ -z "$pkgs" ]; then
        echo "No packages have changes to release"
        exit 0
    fi
    echo "Would release:$pkgs"
    cargo release {{LEVEL}} $pkgs

# Clean everything
clean:
    cargo clean
