#!/usr/bin/env just --justfile

# Using Just: https://github.com/casey/just?tab=readme-ov-file#installation

export RUST_BACKTRACE := "1"
export RUST_LOG := "debug"

# List all of the available commands.
default:
  just --list

# Install any required dependencies.
setup:
	# Make sure the WASM target is installed.
	rustup target add wasm32-unknown-unknown

	# Make sure the right components are installed.
	rustup component add rustfmt clippy

	# Install cargo shear
	cargo install cargo-shear

	# Install cargo sort
	cargo install cargo-sort

	# Install cargo-upgrades and cargo-edit
	cargo install cargo-upgrades cargo-edit

# Run the relay, web server, and publish bbb.
all:
	# Compile the relay server first so it'll win the race.
	cargo build --bin moq-relay

	# Run the relay, web server, and publish bbb.
	pnpm i && npx concurrently --kill-others --names srv,bbb,web --prefix-colors auto "just relay" "just bbb" "just web"

# Run a localhost relay server
relay:
	cargo run --bin moq-relay -- --bind "[::]:4443" --tls-self-sign "localhost:4443" --cluster-node "localhost:4443" --tls-disable-verify

# Run a localhost leaf server, connecting to the relay server
leaf:
	cargo run --bin moq-relay -- --bind "[::]:4444" --tls-self-sign "localhost:4444" --cluster-node "localhost:4444" --cluster-root "localhost:4443" --tls-disable-verify

# Run a cluster of relay servers
cluster:
	pnpm i && npx concurrently --kill-others --names root,leaf,bbb,web --prefix-colors auto "just relay" "sleep 1 && just leaf" "sleep 2 && just bbb" "sleep 3 && just web"

# Download and stream the Big Buck Bunny video to the localhost relay server
bbb: (download "bbb" "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4") (pub "bbb")

# Download and stream the Big Buck Bunny video to localhost directly
bbb-serve: (download "bbb" "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4") (pub-serve "bbb")

# Download and stream the inferior Tears of Steel video
tos: (download "tos" "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/TearsOfSteel.mp4") (pub "tos")

# Download and stream AV1 content:
av1: (download "av1" "http://download.opencontent.netflix.com.s3.amazonaws.com/AV1/Sparks/Sparks-5994fps-AV1-10bit-1920x1080-2194kbps.mp4") (pub "av1")

# Download and stream HEVC content:
hevc: (download "hevc" "https://test-videos.co.uk/vids/jellyfish/mp4/h265/1080/Jellyfish_1080_10s_30MB.mp4") (pub "hevc")

# Download the video and convert it to a fragmented MP4 that we can stream
download name url:
	if [ ! -f dev/{{name}}.mp4 ]; then \
		wget {{url}} -O dev/{{name}}.mp4; \
	fi

	if [ ! -f dev/{{name}}.fmp4 ]; then \
		ffmpeg -i dev/{{name}}.mp4 \
			-c copy \
			-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
			dev/{{name}}.fmp4; \
	fi

# Publish a video using ffmpeg to the localhost relay server
pub name:
	# Pre-build the binary so we don't queue media while compiling.
	cargo build --bin hang

	# Run ffmpeg and pipe the output to hang
	ffmpeg -hide_banner -v quiet \
		-stream_loop -1 -re \
		-i "dev/{{name}}.fmp4" \
		-c copy \
		-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
		- | cargo run --bin hang -- publish "http://localhost:4443/demo/{{name}}"

# Publish a video using ffmpeg directly from hang to the localhost
pub-serve name:
	# Pre-build the binary so we don't queue media while compiling.
	cargo build --bin hang

	# Run ffmpeg and pipe the output to hang
	ffmpeg -hide_banner -v quiet \
		-stream_loop -1 -re \
		-i "dev/{{name}}.fmp4" \
		-c copy \
		-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
		- | cargo run --bin hang -- --bind "[::]:4443" --tls-self-sign "localhost:4443" --tls-disable-verify serve "/demo/{{name}}"

# Run the web server
web:
	pnpm -r i && pnpm -r run dev

# Publish the clock broadcast
clock-pub:
	cargo run --bin moq-clock -- "http://localhost:4443" publish

# Subscribe to the clock broadcast
clock-sub:
	cargo run --bin moq-clock -- "http://localhost:4443" subscribe

# Run the CI checks
check:
	cargo check --all-targets --all-features
	cargo check -p moq-wasm --target wasm32-unknown-unknown
	cargo clippy --all-targets --all-features -- -D warnings
	cargo clippy -p moq-wasm --target wasm32-unknown-unknown
	cargo fmt -- --check

	# requires: cargo install cargo-shear
	cargo shear

	# requires: cargo install cargo-sort
	cargo sort --workspace --check

	# Javascript time
	pnpm -r i

	# Format the JS packages
	biome check

	# Make sure Typescript compiles
	pnpm -r run check

	# Make sure the JS packages are not vulnerable
	pnpm -r exec pnpm audit

	# Beta: Check for unused imports
	pnpm exec knip

# Run any CI tests
test:
	cargo test

# Automatically fix some issues.
fix:
	cargo fix --allow-staged --all-targets --all-features
	cargo clippy --fix --allow-staged --all-targets --all-features
	cargo clippy -p moq-wasm --target wasm32-unknown-unknown --fix --allow-staged --all-targets --all-features
	cargo fmt --all

	# requires: cargo install cargo-shear
	cargo shear --fix

	# requires: cargo install cargo-sort
	cargo sort --workspace

	# Fix the JS packages
	pnpm -r i

	# Format and lint
	biome check --fix --unsafe

	# Make sure the JS packages are not vulnerable
	pnpm -r exec pnpm audit --fix

# Upgrade any tooling
upgrade:
	rustup upgrade

	# Requires: cargo install cargo-upgrades cargo-edit
	cargo upgrade

	# Update the NPM dependencies
	pnpm -r update
	pnpm -r outdated

# Build the release NPM package
build:
	pnpm -r i && pnpm -r run build