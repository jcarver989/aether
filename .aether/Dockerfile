FROM rust:bookworm

# Install system dependencies needed to build the project
# - libssl-dev: for reqwest/tokio TLS
# - libgit2-dev: for git2 crate
# - pkg-config: for linking native libs
# - git: for git operations
# - build-essential: compiler toolchain
RUN apt-get update && apt-get install -y \
    libssl-dev \
    libgit2-dev \
    pkg-config \
    git \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install Rust components
RUN rustup component add clippy rustfmt rust-analyzer

# Set default working directory (will be overlay-mounted at runtime)
WORKDIR /workspace

# Default command - shell for interactive use
CMD ["bash"]
