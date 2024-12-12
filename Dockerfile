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

## build-wasm
FROM oven/bun:slim AS build-wasm
WORKDIR /build

RUN apt-get update && apt-get install -y ca-certificates curl build-essential

# Install Rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup target add wasm32-unknown-unknown
RUN cargo install -f wasm-bindgen-cli

# Install bun dependencies
COPY package.json bun.lockb .
RUN bun install

# Copy the rest
COPY . ./

# Build it
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/build/target \
	bun run rspack

# moq-clock
FROM debian:bookworm-slim AS moq-clock
COPY --from=build /usr/local/cargo/bin/moq-clock /usr/local/bin
ENTRYPOINT ["moq-clock"]

## moq-karp (and moq-bbb)
FROM debian:bookworm-slim AS moq-karp
RUN apt-get update && apt-get install -y ffmpeg wget
COPY ./deploy/moq-bbb /usr/local/bin/moq-bbb
COPY --from=build /usr/local/cargo/bin/moq-karp /usr/local/bin
ENTRYPOINT ["moq-karp"]

## moq-web
FROM caddy:alpine AS moq-web
EXPOSE 443
COPY --from=build-wasm /build/dist /srv

ENTRYPOINT ["caddy", "file-server", "--root", "/srv", "--listen", ":443"]

## moq-relay
FROM debian:bookworm-slim AS moq-relay
COPY --from=build /usr/local/cargo/bin/moq-relay /usr/local/bin
ENTRYPOINT ["moq-relay"]
