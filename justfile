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
    cargo sweep --time 1

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
    cargo sweep --installed

# Release packages whose sources changed since their last tag, plus any
# packages that transitively path-depend on them. Dependents are always
# included because any change to A means B — which path-depends on A —
# resolves to a new A, so B's published artifact changes and B needs its
# own version bump regardless of whether A's change was breaking.
_release LEVEL FLAGS:
    #!/usr/bin/env bash
    set -euo pipefail
    metadata=$(cargo metadata --no-deps --format-version 1)
    seeds=()
    while IFS=$'\t' read -r name dir; do
        tag=$(git tag -l "${name}-v*" --sort=-v:refname | head -1)
        if [ -z "$tag" ] || [ -n "$(git log "${tag}..HEAD" -- "$dir")" ]; then
            seeds+=("$name")
        fi
    done < <(echo "$metadata" | jq -r '.packages[] | [.name, (.manifest_path | sub("/Cargo.toml$"; ""))] | @tsv')
    if [ ${#seeds[@]} -eq 0 ]; then
        echo "No packages have changes to release"
        exit 0
    fi
    pkgs=$(echo "$metadata" | jq -r --args '
        (reduce .packages[] as $p ({};
            reduce ($p.dependencies[] | select(.path != null and .kind != "dev") | .name) as $d
                (.; .[$d] += [$p.name]))) as $rev
        | def close(s):
            ((s + (s | map($rev[.] // []) | add // [])) | unique) as $next
            | if $next == s then s else close($next) end;
          close($ARGS.positional | unique) | map("-p " + .) | join(" ")
    ' -- "${seeds[@]}")
    [ -z "{{FLAGS}}" ] && verb="Would release" || verb="Releasing"
    echo "${verb}: $pkgs"
    cargo release {{LEVEL}} $pkgs {{FLAGS}}

# Release eligible packages
release LEVEL: (_release LEVEL "--execute")

# Dry-run release (no commits, tags, or publishing)
release-dry-run LEVEL="patch": (_release LEVEL "")

# Clean everything
clean:
    cargo clean
