# List available recipes
default:
    @just --list

run:
    cargo run -p wisp -- -a 'cargo run -p aether-cli acp'

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

# Build the sandbox Docker image
build-sandbox:
    cargo build --release -p aether-cli
    cp target/release/aether docker/
    docker build -t aether-sandbox:latest -f docker/Dockerfile.sandbox docker/
    rm docker/aether

# Run inside the sandbox container
run-sandbox *ARGS:
    cargo run -p aether-cli -- --sandbox {{ARGS}}

# Clean everything
clean:
    cargo clean
