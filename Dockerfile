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

# Special image for moq-pub with ffmpeg and a publish script included.
FROM rust:latest as moq-pub

# Install required utilities and ffmpeg
RUN apt-get update && \
    apt-get install -y ffmpeg wget

# Copy the publish script into the image
COPY deploy/publish.sh /usr/local/bin/publish

# Copy the compiled binaries
COPY --from=builder /usr/local/cargo/bin /usr/local/cargo/bin
CMD [ "publish" ]

# moq-rs image with just the binaries (default)
FROM rust:latest as moq-rs

LABEL org.opencontainers.image.source=https://github.com/kixelated/moq-rs
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

# Fly.io entrypoint
ADD deploy/fly-relay.sh .

# Copy the compiled binaries
COPY --from=builder /usr/local/cargo/bin /usr/local/cargo/bin
