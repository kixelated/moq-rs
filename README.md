![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
[![Discord](https://img.shields.io/discord/1124083992740761730)](https://discord.gg/FCYF3p99mr)

<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live (media) delivery protocol utilizing QUIC.
It utilizes new browser technologies such as [WebTransport](https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API) and [WebCodecs](https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API) to provide WebRTC-like functionality.
Despite the focus on media, the transport is generic and designed to scale to enormous viewership via clustered relay servers (aka a CDN).
See [quic.video](https://quic.video) for more information.

**Note:** this project is a [fork](https://quic.video/blog/transfork) of the [IETF specification](https://datatracker.ietf.org/group/moq/documents/).
The principles are the same but the implementation is exponentially simpler given a narrower focus (and no politics).

## Quick Start
Too many words, here's a quick start:

**Requirements:**
- [Rustup](https://www.rust-lang.org/tools/install)
- [Just](https://github.com/casey/just?tab=readme-ov-file#installation)
- [Node + NPM](https://nodejs.org/)

```sh
just setup
just all
```

## Design
For the non-vibe coders, let's talk about how the protocol works.
Unlike WebRTC, MoQ is purposely split into multiple layers to allow for maximum flexibility:

- **QUIC**: The network layer, providing a semi-reliable transport over UDP.
- **WebTransport**: QUIC but with a HTTP/3 handshake for browser compatibility.
- **moq-lite**: A pub/sub protocol used to serve "broadcasts", "tracks", "groups", and "frames".
- **hang**: Media-specific encoding based on the [WebCodecs API](https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API). It's designed primarily for real-time conferencing.
- **application**: Any stuff you want to build on top of moq/hang.

For more information, check out [the blog](https://quic.video/blog/moq-onion).
If you're extra curious, check out the [specification](https://github.com/kixelated/moq-drafts).
If you want to make changes (or argue about anything), ping `@kixelated` on [Discord](https://discord.gg/FCYF3p99mr).


## Languages
This repository is split based on the language and environment:
- [rs](rs): Rust libraries for native platforms. (and WASM)
- [js](js): Typescript libraries for web only. (not node)

Unfortunately, this means that there are two implementations of `moq` and `hang`.
They use the same concepts and have a similar API but of course there are language differences.

Originally, this repository was called `moq-rs` with the aim of using Rust to target all platforms.
Unfortunately, it turns out that much of the code is platform specific, so it resulted in a tiny Rust core and a lot of convoluted wrappers.

For example, a MoQ web player MUST use browser APIs for networking, encoding, decoding, rendering, capture, etc.
The web is a sandbox, and with no low level hardware access we have to jump through the APIs designed for Javascript.
[wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/) and [web-sys](https://rustwasm.github.io/wasm-bindgen/api/web_sys/) help, and you can see an example in [hang-wasm](rs/hang-wasm), but it's a lot of boilerplate and language juggling for little gain.

Anyway, enough ranting, let's get to the code.

### Rust (Native)
- [moq](https://docs.rs/moq-lite): The underlying pub/sub protocol providing real-time latency and scale. This is a simplified fork of the IETF [moq-transport draft](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/) called `moq-lite`.
- [moq-relay](rs/moq-relay): A server that forwards content from publishers to any interested subscribers. It can be clustered, allowing multiple servers to be run (around the world!) that will proxy content to each other.
- [moq-clock](rs/moq-clock): A dumb clock client/server just to prove MoQ can be used for more than media.
- [moq-native](rs/moq-native): Helpers to configure QUIC on native platforms.
- [hang](https://docs.rs/hang): Media-specific components built on top of MoQ.
- [hang-cli](rs/hang-cli): A CLI for publishing and subscribing to media.
- [hang-gst](rs/hang-gst): A gstreamer plugin for publishing and subscribing to media.
- [web-transport](https://github.com/kixelated/web-transport-rs): A Rust implementation of the WebTransport API powered by [Quinn](https://github.com/quinn-rs/quinn).

The [hang-wasm](rs/hang-wasm) crate is currently unmaintained and depends on some other unmaintained crates. Notably:
- [web-transport-wasm](https://github.com/kixelated/web-transport-rs/tree/main/web-transport-wasm): A wrapper around the WebTransport API.
- [web-codecs](https://docs.rs/web-codecs/latest/web_codecs/): A wrapper around the WebCodecs API.
- [web-streams](https://docs.rs/web-streams/latest/web_streams/): A wrapper around the WebStreams API.
- You get the idea; it's wrappers all the way down.


### Typescript (Web)
- [@kixelated/moq](https://www.npmjs.com/package/@kixelated/moq): The underlying pub/sub protocol providing real-time latency and scale.
- [@kixelated/hang](https://www.npmjs.com/package/@kixelated/hang): Media-specific components built on top of MoQ. Can be embedded on a web page via a Web Component.

Documentation is sparse as the API is still a work in progress.
Check out the [demo](js/hang/src/demo) for an example.


# Development
We use [just](https://github.com/casey/just) to simplify the development process.
Check out the [justfile](justfile) or run `just` to see the available commands.

## Requirements
- [Rustup](https://www.rust-lang.org/tools/install)
- [Node](https://nodejs.org/)
- [Just](https://github.com/casey/just?tab=readme-ov-file#installation)

Install any other required tools:
```sh
just setup
```

## Building

```sh
# Run the relay, a demo movie, and web server:
just all

# Or run each individually in separate terminals:
just relay
just pub bbb
just web
```

Then, visit [https://localhost:8080](localhost:8080) to watch the simple demo.

### Contributing
When you're ready to submit a PR, make sure the tests pass or face the wrath of CI:
```sh
just check

# Optional: Automatically fix easy lint issues.
just fix
```


# License

Licensed under either:
-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
