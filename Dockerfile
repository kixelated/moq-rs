FROM rust:latest as builder

# Make a fake Rust app to keep a cached layer of compiled crates
RUN USER=root cargo new app
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./

RUN mkdir -p moq-transport/src moq-quinn/src moq-warp/src moq-pub/src
COPY moq-transport/Cargo.toml moq-transport/Cargo.toml
COPY moq-quinn/Cargo.toml moq-quinn/Cargo.toml
COPY moq-pub/Cargo.toml moq-pub/Cargo.toml
COPY moq-warp/Cargo.toml moq-warp/Cargo.toml
RUN touch moq-transport/src/lib.rs
RUN touch moq-warp/src/lib.rs
RUN touch moq-pub/src/lib.rs
RUN touch moq-quinn/src/lib.rs

RUN sed -i '/default-run.*/d' moq-quinn/Cargo.toml

# Will build all dependent crates in release mode
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo build --release

# Copy the rest
COPY . .

# Build (install) the actual binaries
RUN cargo install --path moq-quinn

# Runtime image
FROM rust:latest

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /usr/local/cargo/bin/moq-quinn /app/moq-quinn

ADD entrypoint.sh .
# No CMD or ENTRYPOINT, see fly.toml with `cmd` override.
