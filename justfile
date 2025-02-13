#!/usr/bin/env just --justfile

# Using Just: https://github.com/casey/just?tab=readme-ov-file#installation

export RUST_BACKTRACE := "1"
export RUST_LOG := "info"

# List all of the available commands.
default:
  just --list

# Install any required dependencies.
setup:
	# Make sure the WASM target is installed.
	rustup target add wasm32-unknown-unknown

	# Make sure the right components are installed.
	rustup component add rustfmt clippy

	# Install cargo binstall if needed.
	if ! command -v cargo-binstall > /dev/null; then \
		cargo install cargo-binstall; \
	fi

	# Install cargo shear if needed.
	if ! command -v cargo-shear > /dev/null; then \
		cargo binstall --no-confirm cargo-shear; \
	fi

# Run the relay, web server, and publish bbb.
all:
	npm i && npx concurrently --kill-others --names srv,bbb,web --prefix-colors auto "just relay" "sleep 1 && just bbb" "sleep 2 && just web"

# Run a localhost relay server
relay:
	cargo run --bin moq-relay -- --bind "[::]:4443" --tls-self-sign "localhost:4443" --cluster-node "localhost:4443" --tls-disable-verify --dev

# Run a localhost leaf server, connecting to the relay server
leaf:
	cargo run --bin moq-relay -- --bind "[::]:4444" --tls-self-sign "localhost:4444" --cluster-node "localhost:4444" --cluster-root "localhost:4443" --tls-disable-verify --dev

# Run a cluster of relay servers
cluster:
	npm i && npx concurrently --kill-others --names root,leaf,bbb,web --prefix-colors auto "just relay" "sleep 1 && just leaf" "sleep 2 && just bbb" "sleep 3 && just web"

# Download and stream the Big Buck Bunny video
bbb: (download "bbb" "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4") (pub "bbb")

# Download and stream the inferior Tears of Steel video
tos: (download "tos" "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/TearsOfSteel.mp4") (pub "tos")

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
	cargo build --bin moq-karp

	# Run ffmpeg and pipe the output to moq-karp
	ffmpeg -hide_banner -v quiet \
		-stream_loop -1 -re \
		-i "dev/{{name}}.fmp4" \
		-c copy \
		-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
		- | cargo run --bin moq-karp -- publish "http://localhost:4443/demo/{{name}}"

# Run the web server
web:
	npm i && npm run dev

# Publish the clock broadcast
clock-pub:
	cargo run --bin moq-clock -- "http://localhost:4443" publish

# Subscribe to the clock broadcast
clock-sub:
	cargo run --bin moq-clock -- "http://localhost:4443" subscribe

# Run the CI checks
check:
	cargo check --all-targets
	cargo clippy --all-targets -- -D warnings
	cargo fmt -- --check
	cargo shear # requires: cargo binstall cargo-shear
	npm i && npm run check

# Run any CI tests
test:
	cargo test

# Automatically fix some issues.
fix:
	cargo fix --all --allow-staged --all-targets --all-features
	cargo clippy --all --fix --allow-staged --all-targets --all-features
	cargo fmt --all
	npm i && npm run fix
	cargo shear --fix

# Build the release NPM package
build:
	npm i && npm run build

# Build and link the NPM package
link:
	npm i && npm run build:dev && npm run build:tsc
	npm link

# Delete any ephemeral build files
clean:
	rm -r dist
