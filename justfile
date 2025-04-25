#!/usr/bin/env just --justfile

# Using Just: https://github.com/casey/just?tab=readme-ov-file#installation

export RUST_BACKTRACE := "1"
export RUST_LOG := "debug"
#export GST_DEBUG:="*:4"

# List all of the available commands.
default:
  just --list

# Install any required dependencies.
setup:
	# Install binstall so we can install the other tools faster.
	cargo install cargo-binstall

	# Install the tools
	just setup-tools

# CI calls this script directly because it gets rust/binstall from other sources.
setup-tools:
	# Install utilities
	cargo binstall -y cargo-shear cargo-sort cargo-upgrades cargo-edit cargo-audit

# Run the relay, web server, and publish bbb.
all:
	# Run the relay, web server, and publish bbb.
	pnpm i && pnpm exec concurrently --kill-others --names srv,bbb,web --prefix-colors auto "just relay" "sleep 1 && just bbb" "sleep 2 && just web"

# Alternatively, build the demo and host it via hang-cli.
# This avoids multiple binaries and complicating things.
demo: build
	just serve-bbb

# Run a localhost relay server
relay:
	cargo run --bin moq-relay -- --bind "[::]:4443" --tls-self-sign "localhost:4443" --cluster-node "localhost:4443" --tls-disable-verify

# Run a localhost leaf server, connecting to the relay server
leaf:
	cargo run --bin moq-relay -- --bind "[::]:4444" --tls-self-sign "localhost:4444" --cluster-node "localhost:4444" --cluster-root "localhost:4443" --tls-disable-verify

# Run a cluster of relay servers
cluster:
	pnpm i && pnpm exec concurrently --kill-others --names root,leaf,bbb,web --prefix-colors auto "just relay" "sleep 1 && just leaf" "sleep 2 && just bbb" "sleep 3 && just web"


# Download the video and convert it to a fragmented MP4 that we can stream
download name:
	@url=$(just download-url {{name}})
	mkdir -p dev

	if [ ! -f dev/{{name}}.mp4 ]; then \
		wget "$url" -O dev/{{name}}.mp4; \
	fi

	if [ ! -f dev/{{name}}.fmp4 ]; then \
		ffmpeg -i dev/{{name}}.mp4 \
			-c copy \
			-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
			dev/{{name}}.fmp4; \
	fi

# Returns the URL for a test video.
download-url name:
	@case {{name}} in \
		bbb) echo "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4" ;; \
		tos) echo "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/TearsOfSteel.mp4" ;; \
		av1) echo "http://download.opencontent.netflix.com.s3.amazonaws.com/AV1/Sparks/Sparks-5994fps-AV1-10bit-1920x1080-2194kbps.mp4" ;; \
		hevc) echo "https://test-videos.co.uk/vids/jellyfish/mp4/h265/1080/Jellyfish_1080_10s_30MB.mp4" ;; \
		*) echo "unknown" && exit 1 ;; \
	esac

# Publish a video using ffmpeg to the localhost relay server
pub name:
	# Download the sample media.
	just download {{name}}

	# Pre-build the binary so we don't queue media while compiling.
	cargo build --bin hang

	# Run ffmpeg and pipe the output to hang
	ffmpeg -hide_banner -v quiet \
		-stream_loop -1 -re \
		-i "dev/{{name}}.fmp4" \
		-c copy \
		-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
		- | cargo run --bin hang -- publish "http://localhost:4443/demo/{{name}}"

# Publish a video using gstreamer to the localhost relay server
pub-gst name:
	# Download the sample media.
	just download {{name}}

	# Build the plugins
	cargo build

	# Run gstreamer and pipe the output to our plugin
	GST_PLUGIN_PATH="${PWD}/target/debug${GST_PLUGIN_PATH:+:$GST_PLUGIN_PATH}" \
	gst-launch-1.0 -v -e multifilesrc location="dev/{{name}}.fmp4" loop=true ! qtdemux name=demux \
		demux.video_0 ! h264parse ! queue ! identity sync=true ! isofmp4mux name=mux chunk-duration=1 fragment-duration=1 ! hangsink url="http://localhost:4443/demo/{{name}}" tls-disable-verify=true \
		demux.audio_0 ! aacparse ! queue ! mux.

# Subscribe to a video using gstreamer
sub name:
	# Build the plugins
	cargo build

	# Run gstreamer and pipe the output to our plugin
	# This will render the video to the screen
	GST_PLUGIN_PATH="${PWD}/target/debug${GST_PLUGIN_PATH:+:$GST_PLUGIN_PATH}" \
	gst-launch-1.0 -v -e hangsrc url="http://localhost:4443/demo/{{name}}" tls-disable-verify=true ! decodebin ! videoconvert ! autovideosink

# Publish a video using ffmpeg directly from hang to the localhost
serve name:
	# Download the sample media.
	just download {{name}}

	# Pre-build the binary so we don't queue media while compiling.
	cargo build --bin hang

	# Run ffmpeg and pipe the output to hang
	ffmpeg -hide_banner -v quiet \
		-stream_loop -1 -re \
		-i "dev/{{name}}.fmp4" \
		-c copy \
		-f mp4 -movflags cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame \
		- | cargo run --bin hang -- --bind "[::]:4443" --tls-self-sign "localhost:4443" --tls-disable-verify serve "demo/{{name}}"

# Run the web server
web:
	pnpm -r i
	RUST_LOG=warn pnpm -r run dev

# Publish the clock broadcast
# `action` is either `publish` or `subscribe`
clock action:
	if [ "{{action}}" != "publish" ] && [ "{{action}}" != "subscribe" ]; then \
		echo "Error: action must be 'publish' or 'subscribe', got '{{action}}'" >&2; \
		exit 1; \
	fi

	cargo run --bin moq-clock -- "http://localhost:4443" {{action}}

# Run the CI checks
check flags="":
	cargo check --all-targets --all-features {{flags}}
	cargo clippy --all-targets --all-features {{flags}} -- -D warnings
	cargo fmt --all --check

	# Make sure it actually compiles with WASM.
	cargo check -p hang-wasm --target wasm32-unknown-unknown

	# requires: cargo install cargo-shear
	cargo shear

	# requires: cargo install cargo-sort
	cargo sort --workspace --check

	# requires: cargo install cargo-audit
	cargo audit

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

# Automatically fix some issues.
fix flags="":
	cargo fix --allow-staged --all-targets --all-features {{flags}}
	cargo clippy --fix --allow-staged --all-targets --all-features {{flags}}
	cargo fmt --all

	# requires: cargo install cargo-shear
	cargo shear --fix

	# requires: cargo install cargo-sort
	cargo sort --workspace

	# requires: cargo install cargo-audit
	cargo audit

	# Update any patch versions
	cargo update

	# Fix the JS packages
	pnpm -r i

	# Format and lint
	biome check --fix --unsafe

	# Make sure the JS packages are not vulnerable
	pnpm -r exec pnpm audit --fix

# Run any CI tests
test:
	cargo test

# Upgrade any tooling
upgrade:
	# Upgrade the Rust toolchain... I guess.
	rustup upgrade

	# Update any patch versions
	cargo update

	# Requires: cargo install cargo-upgrades cargo-edit
	cargo upgrade --incompatible

	# Update the NPM dependencies
	pnpm self-update
	pnpm -r update
	pnpm -r outdated

# Build the packages
build:
	cargo build

	pnpm -r i
	RUST_LOG=warn pnpm -r run build
