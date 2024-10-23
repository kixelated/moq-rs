FROM rust:bookworm AS builder

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
FROM debian:bookworm-slim

# Install required utilities and ffmpeg
RUN apt-get update && \
    apt-get install -y ffmpeg wget

# Copy the publish script into the image
COPY ./deploy/moq-bbb /usr/local/bin/moq-bbb

# Copy over the built binaries.
COPY --from=builder /usr/local/cargo/bin/moq-* /usr/local/bin

# Default to moq-relay
CMD ["moq-relay"]
