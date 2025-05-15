<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
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

## Layers
Now that those with ADHD have left, let's talk about the layers.
Unlike WebRTC, MoQ is purposely split into multiple layers to allow for maximum flexibility.
For more information, see [the blog](https://quic.video/blog/moq-onion).

- **QUIC**: The network layer, providing a semi-reliable transport over UDP.
- **WebTransport**: QUIC but with a HTTP/3 handshake for browser compatibility.
- **moq-lite**: A pub/sub protocol used to serve "broadcasts", "tracks", "groups", and "frames".
- **hang**: Media-specific encoding based on the [WebCodecs API](https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API). It's designed primarily for real-time conferencing.
- **application**: Any stuff you want to build on top of moq/hang.

If you're curious about the underlying protocols, check out the [specification](https://github.com/kixelated/moq-drafts).
If you want to argue about anything, participate in the [IETF](https://datatracker.ietf.org/group/moq/documents/) or bad-mouth `@kixelated` on [Discord](https://discord.gg/FCYF3p99mr).


## Environments
Originally, this repository was called `moq-rs` with the aim of targetting all platforms.
The reality is that the `moq` and `hang` are relatively simple while everything else is platform specific.

For example, a MoQ web player MUST use browser APIs for networking, encoding, decoding, rendering, capture, etc.
The low level hardware and syscalls otherwise necessary are not exposed to the application (Javascript and WASM alike).
You can use [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/) and [web-sys](https://rustwasm.github.io/wasm-bindgen/api/web_sys/) shims, and you can see an example in [hang-wasm](rs/hang-wasm), but it's a lot of boilerplate and type juggling.

Instead, this repository is split based on the language and environment:
- [rs](rs): Rust libraries for native platforms. (and WASM)
- [js](js): Typescript libraries for web only. (not node)

Unfortunately, this means that there are two implementations of `moq` and `hang`.
They have a similar API but of course there are language differences.

### Rust (Native)

- [moq](rs/moq): The underlying pub/sub protocol providing real-time latency and scale. This is a simplified fork of the IETF [moq-transport draft](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/) called `moq-lite`.
- [moq-relay](rs/moq-relay): A server that forwards content from publishers to any interested subscribers. It can optionally be clustered, allowing N servers to transfer between themselves.
- [moq-clock](rs/moq-clock): A dumb clock client/server just to prove MoQ can be used for more than media.
- [moq-native](rs/moq-native): Helpers to configure QUIC on native platforms.
- [hang](rs/hang): Media-specific components built on top of MoQ.
- [hang-cli](rs/hang-cli): A CLI for publishing and subscribing to media.
- [hang-gst](rs/hang-gst): A gstreamer plugin for publishing and subscribing to media.
- [hang-web](rs/hang-wasm): A web client written in Rust and using WASM.
- [web-transport](https://github.com/kixelated/web-transport-rs): A Rust implementation of the WebTransport API powered by [Quinn](https://github.com/quinn-rs/quinn) (or WASM).


### Typescript (Web)
- [moq](js/moq): The underlying pub/sub protocol providing real-time latency and scale. This is a simplified fork of the IETF [moq-transport draft](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/) called `moq-lite`.
- [hang](js/hang): Media-specific components built on top of MoQ. Can be embedded on a web page via a WebComponent.


# Usage
## Requirements
- [Rustup](https://www.rust-lang.org/tools/install)
- [Just](https://github.com/casey/just?tab=readme-ov-file#installation)
- [Node + NPM](https://nodejs.org/)

## Development
### Setup
We use `just` to simplify the development process.
Check out the [Justfile](justfile) or run `just` to see the available commands.

Install any other required tools:
```sh
just setup
```

### Building

```sh
# Run the relay, a demo movie, and web server:
just all

# Or run each individually in separate terminals:
just relay
just pub bbb
just web
```

Then, visit [https://localhost:8080](localhost:8080) to watch the simple demo.

There are a lot more commands; check out the [Justfile](justfile) and individual README files.

### Contributing
When you're ready to submit a PR, make sure the tests pass or face the wrath of CI:
```sh
just check
just test

# Optional: Automatically fix easy lint issues.
just fix
```


# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
