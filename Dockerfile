FROM rust:latest as builder

# Make a fake Rust app to keep a cached layer of compiled crates
RUN USER=root cargo new app
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./

RUN mkdir -p moq-transport/src moq-transport-quinn/src moq-demo/src moq-warp/src
COPY moq-transport/Cargo.toml moq-transport/Cargo.toml
COPY moq-transport-quinn/Cargo.toml moq-transport-quinn/Cargo.toml
COPY moq-demo/Cargo.toml moq-demo/Cargo.toml
COPY moq-warp/Cargo.toml moq-warp/Cargo.toml
RUN touch moq-transport/src/lib.rs
RUN touch moq-transport-quinn/src/lib.rs
RUN touch moq-demo/src/lib.rs
RUN touch moq-warp/src/lib.rs

# Will build all dependent crates in release mode
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo build --release

# Copy the rest
COPY . .
# Build (install) the actual binaries
RUN cargo install --path moq-demo

# Runtime image
FROM debian:bullseye-slim

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /usr/local/cargo/bin/moq-demo /app/moq-demo

ADD entrypoint.sh .
# No CMD or ENTRYPOINT, see fly.toml with `cmd` override.
