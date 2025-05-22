FROM ubuntu:24.04 AS build

WORKDIR /build
ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTFLAGS=--cfg=web_sys_unstable_apis

# Install base tools and GStreamer
RUN apt-get update && \
    apt-get install -y \
    build-essential \
    curl \
    sudo \
    pkg-config \
    libssl-dev \
    libclang-dev \
    cmake \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-bad1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-tools \
    gstreamer1.0-x \
    gstreamer1.0-alsa \
    gstreamer1.0-gl \
    gstreamer1.0-gtk3 \
    gstreamer1.0-qt5 \
    gstreamer1.0-pulseaudio \
    git \
    unzip \
	just \
    && apt-get clean

# Install rustup + toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Add wasm target + clippy + rustfmt
RUN rustup target add wasm32-unknown-unknown && \
    rustup component add clippy rustfmt

# Let just do the rest.
COPY justfile .

# Install dependencies.
# I'm not sure if all of these --mount flags are needed any longer.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/build/target \
	just setup

# Copy over the rest of the files.
COPY . .

# Make sure u guys don't write bad code before we build.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/build/target \
	just check --release

# Reuse a cache between builds.
# I tried to `cargo install`, but it doesn't seem to work with workspaces.
# There's also issues with the cache mount since it builds into /usr/local/cargo/bin
# We can't mount that without clobbering cargo itself.
# We instead we build the binaries and copy them to the cargo bin directory.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/build/target \
	mkdir -p /out && \
	cargo build --release && \
	cp /build/target/release/moq* /out && \
	cp /build/target/release/hang* /out

# moq-clock
FROM ubuntu:24.04 AS moq-clock
COPY --from=build /out/moq-clock /usr/local/bin
ENTRYPOINT ["moq-clock"]

## hang (and hang-bbb)
FROM ubuntu:24.04 AS hang
RUN apt-get update && apt-get install -y ffmpeg wget
COPY ./hang-bbb /usr/local/bin/hang-bbb
COPY --from=build /out/hang /usr/local/bin
ENTRYPOINT ["hang"]

## moq-relay
FROM ubuntu:24.04 AS moq-relay
COPY --from=build /out/moq-relay /usr/local/bin
ENTRYPOINT ["moq-relay"]
