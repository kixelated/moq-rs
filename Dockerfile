FROM rust:latest as builder

# Create a build directory and copy over all of the files
WORKDIR /build
COPY . .

# Reuse a cache between builds.
# I tried to `cargo install`, but it doesn't seem to work with workspaces.
# There's also issues with the cache mount since it builds into /usr/local/cargo/bin, and we can't mount that without clobbering cargo itself.
# We instead we build the binaries and copy them to the cargo bin directory.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && cp /build/target/release/moq-* /usr/local/cargo/bin

# Runtime image
FROM rust:latest

LABEL org.opencontainers.image.source=https://github.com/kixelated/moq-rs
LABEL org.opencontainers.image.licenses=MIT OR Apache-2.0

# Fly.io entrypoint
ADD fly-relay.sh .

# Copy the compiled binaries
COPY --from=builder /usr/local/cargo/bin /usr/local/cargo/bin
