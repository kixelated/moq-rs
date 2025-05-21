FROM rust:slim-bookworm AS build

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

# moq-clock
FROM debian:bookworm-slim AS moq-clock
COPY --from=build /usr/local/cargo/bin/moq-clock /usr/local/bin
ENTRYPOINT ["moq-clock"]

## hang (and hang-bbb)
FROM debian:bookworm-slim AS hang
RUN apt-get update && apt-get install -y ffmpeg wget
COPY ./hang-bbb /usr/local/bin/hang-bbb
COPY --from=build /usr/local/cargo/bin/hang /usr/local/bin
ENTRYPOINT ["hang"]

## moq-relay
FROM debian:bookworm-slim AS moq-relay
COPY --from=build /usr/local/cargo/bin/moq-relay /usr/local/bin
ENTRYPOINT ["moq-relay"]
