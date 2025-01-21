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
- [moq-web](moq-web): A web client utilizing Rust and WASM. Supports both consuming media (and soon publishing).
-   [moq-transfork](moq-transfork): The underlying network protocol. It can be used by live applications that need real-time and scale, even if they're not media.
- [moq-karp](moq-karp): The underlying media protocol powered by moq-transfork. It includes a CLI for importing/exporting to other formats, for example integrating with ffmpeg.
- [moq-gst](moq-gst): A gstreamer plugin for producing Karp broadcasts. Note: ffmpeg is supported via moq-karp directly.
-   [moq-clock](moq-clock): A dumb clock client/server just to prove MoQ can be used for more than media.
-   [moq-native](moq-native): Helpers to configure the native MoQ tools.



# Usage
## Requirements
- [Rust](https://www.rust-lang.org/tools/install) + WASM target
- [Node + NPM](https://nodejs.org/)

```sh
rustup target add wasm32-unknown-unknown
```

## Development
There's a few scripts in the [dev](dev) directory to help you get started.
You can run them directly in separate terminals or use the `all` script to run them all at once.

```sh
# Run the relay, a demo movie, and web server:
./dev/all

# Or run each individually in separate terminals:
./dev/relay
./dev/bbb
./dev/web
```

Then, visit [https://localhost:8080](localhost:8080) to watch the simple demo.


# Components
## moq-relay

[moq-relay](moq-relay) is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
It's designed to be run in a datacenter, relaying media across multiple hops to deduplicate and improve QoS.

Notable arguments:

-   `--bind <ADDR>` Listen on this address, default: `[::]:4443`
-   `--tls-cert <CERT>` Use the certificate file at this path
-   `--tls-key <KEY>` Use the private key at this path
-   `--announce <URL>` Forward all announcements to this address, typically a root relay.

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

## moq-web

[moq-web](moq-web) is a web client that can consume media (and soon publish).
It's available [on NPM](https://www.npmjs.com/package/@kixelated/moq) as both a JS library and web component.

For example:

```html
<script type="module">
	import '@kixelated/moq/video'
</script>

<moq-video src="https://relay.quic.video/demo/bbb"></moq-video>
```

The package is a gross frankenstein of Rust+Typescript.
To run the demo page:

```sh
npm i
npm run dev
```

You can also test the package locally by linking.
Replace `npm` with your favorite package manager (ex. npm, yarn, bun); it might work.

```sh
npm run build
npm link

# In your other package
npm link @kixelated/moq
```

See the [moq-web README](moq-web/README.md) for more information.

## moq-karp

[moq-karp](moq-karp) is a simple media layer on top of MoQ.
The crate includes a binary that accepts fMP4 with a few restrictions:

-   `separate_moof`: Each fragment must contain a single track.
-   `frag_keyframe`: A keyframe must be at the start of each keyframe.
-   `fragment_per_frame`: (optional) Each frame should be a separate fragment to minimize latency.

This can be used in conjunction with ffmpeg to publish media to a MoQ relay.
See [dev/pub](dev/pub) for the required ffmpeg flags.

Alternatively, see [moq-gst](./moq-gst) for a gstreamer plugin.

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
