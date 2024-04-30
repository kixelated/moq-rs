FROM rust:bookworm as builder

# Create a build directory and copy over all of the files
WORKDIR /build
COPY . ./

# Reuse a cache between builds.
# I tried to `cargo install`, but it doesn't seem to work with workspaces.
# There's also issues with the cache mount since it builds into /usr/local/cargo/bin
# We can't mount that without clobbering cargo itself.
# We instead we build the binaries and copy them to the cargo bin directory.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && cp /build/target/release/moq-* /usr/local/cargo/bin

# Create a pub image that also contains ffmpeg and a helper script
FROM debian:bookworm-slim as moq-pub

# Install required utilities and ffmpeg
RUN apt-get update && \
    apt-get install -y ffmpeg wget

# Copy the publish script into the image
COPY ./deploy/publish /usr/local/bin/publish

# Copy over the built binaries.
COPY --from=builder /usr/local/cargo/bin/moq-* /usr/local/bin

# Use our publish script
CMD [ "publish" ]

# Create an image with just the binaries
FROM debian:bookworm-slim

RUN apt-get update && \
	apt-get install -y --no-install-recommends ca-certificates curl libssl3 && \
	rm -rf /var/lib/apt/lists/*

LABEL org.opencontainers.image.source=https://github.com/kixelated/moq-rs
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

COPY --from=builder /usr/local/cargo/bin/moq-* /usr/local/bin

# Entrypoint to load relay TLS config in Fly
# TODO remove this; it should be specific to the fly deployment.
COPY deploy/fly-relay.sh .

# Default to moq-relay
CMD ["moq-relay"]
