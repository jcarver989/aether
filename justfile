# List available recipes
default:
    @just --list

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

# Sweep build artifacts older than N days (default: 7)
sweep DAYS="7":
    cargo sweep --time {{DAYS}}

# Sweep artifacts not used by the current toolchain
sweep-installed:
    cargo sweep --installed

# Clean everything
clean:
    cargo clean
