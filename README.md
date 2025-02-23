<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC.
It's a client-server model that is designed to scale to enormous viewership via clustered relay servers (aka a CDN).
The application determines the trade-off between latency and quality, potentially on a per viewer-basis.

See [quic.video](https://quic.video) for more information.
Note: this project is a [fork of the IETF draft](https://quic.video/blog/transfork) to speed up development.
If you're curious about the protocol, check out the current [specification](https://github.com/kixelated/moq-drafts).

The project is split into a few crates:

-   [moq-relay](moq-relay): A server that forwards content from publishers to any interested subscribers. It can optionally be clustered, allowing N servers to transfer between themselves.
- [moq-web](moq-web): A web client utilizing Rust and WASM. Supports both consuming and publishing media.
-   [moq-transfork](moq-transfork): The underlying network protocol. It can be used by live applications that need real-time and scale, even if they're not media.
- [moq-karp](moq-karp): The underlying media protocol powered by moq-transfork. It includes a CLI for importing/exporting to other formats, for example integrating with ffmpeg.
-   [moq-clock](moq-clock): A dumb clock client/server just to prove MoQ can be used for more than media.
-   [moq-native](moq-native): Helpers to configure the native MoQ tools.



# Usage
## Requirements
- [Rustup](https://www.rust-lang.org/tools/install)
- [Just](https://github.com/casey/just?tab=readme-ov-file#installation)
- [Node + NPM](https://nodejs.org/)

## Setup
We use `just` to simplify the development process.
Check out the [Justfile](justfile) or run `just` to see the available commands.

Install any other required tools:
```sh
just setup
```

## Development

```sh
# Run the relay, a demo movie, and web server:
just all

# Or run each individually in separate terminals:
just relay
just bbb
just web
```

Then, visit [https://localhost:8080](localhost:8080) to watch the simple demo.

When you're ready to submit a PR, make sure the tests pass or face the wrath of CI:
```sh
just check
just test
```

# Components
## moq-relay

[moq-relay](moq-relay) is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
It's designed to be run in a datacenter, relaying media across multiple hops to deduplicate and improve QoS.

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

## moq-web

[moq-web](moq-web) is a web client that can consume media (and soon publish).
It's available [on NPM](https://www.npmjs.com/package/@kixelated/moq) as both a JS library and web component.

For example:

```html
<script type="module">
	import '@kixelated/moq/watch'
</script>

<moq-watch url="https://relay.quic.video/demo/bbb"></moq-watch>
```


See the [moq-web README](moq-web/README.md) for more information.

## moq-karp

[moq-karp](moq-karp) is a simple media layer on top of MoQ.
The crate includes a binary that accepts fMP4 with a few restrictions:

-   `separate_moof`: Each fragment must contain a single track.
-   `frag_keyframe`: A keyframe must be at the start of each keyframe.
-   `fragment_per_frame`: (optional) Each frame should be a separate fragment to minimize latency.

This can be used in conjunction with ffmpeg to publish media to a MoQ relay.
See the [Justfile](./justfile) for the required ffmpeg flags.

Alternatively, see [moq-gst](https://github.com/kixelated/moq-gst) for a gstreamer plugin.

## moq-transfork

A media-agnostic library used by [moq-relay](moq-relay) and [moq-karp](moq-karp) to serve the underlying subscriptions.
It has caching/deduplication built-in, so your application is oblivious to the number of connections under the hood.

See the published [crate](https://crates.io/crates/moq-transfork) and [documentation](https://docs.rs/moq-transfork/latest/moq_transfork/).

## moq-clock

[moq-clock](moq-clock) is a simple client that can publish or subscribe to the current time.
It's meant to demonstate that [moq-transfork](moq-transfork) can be used for more than just media.

## nix/nixos

moq also has nix support see [`nix/README.md`](nix/README.md)


# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
