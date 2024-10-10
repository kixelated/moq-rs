<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC.
It's a client-server model (not peer-to-peer) that is designed to scale to enormous viewership via clustered relay servers (aka a CDN).
The application determines the trade-off between latency and quality, potentially on a per viewer-basis.

See [quic.video](https://quic.video) for more information.
Note: this project uses a forked implementation of the IETF standardization effort.

The project is split into a few crates:

-   [moq-transfork](moq-transfork): The underlying network protocol. It's designed for media-like applications that need real-time and scale.
-   [moq-relay](moq-relay): A server that forwards content from publishers to any interested subscribers. It can optionally be clustered, allowing N servers to transfer between themselves.
- [moq-karp](moq-karp): A simple media layer powered by moq-transfork, intended as a HLS/DASH/SDP replacement. It includes a CLI for converting between formats.
-   [moq-clock](moq-clock): A dumb clock client/server just to prove MoQ can be used for more than media.
-   [moq-native](moq-native): Helpers to configure the native MoQ tools.

There are additional components that have been split into other repositories for development reasons:

- [moq-gst](https://github.com/kixelated/moq-gst): A gstreamer plugin for producing Karp broadcasts.
- [moq-wasm](https://github.com/kixelated/moq-wasm): A web client utilizing Rust and WASM.
- [moq-js](https://github.com/kixelated/moq-js): A web client utilizing Typescript.

At the moment [moq-js](https://github.com/kixelated/moq-js) contains the only mature player.
A hosted version is available at [quic.video](https://quic.video) and you can use the `?host=localhost:4443` query parameter to target a local moq-relay.

# Usage
For quick iteration cycles, use the [dev helper scripts](dev/README.md).

To launch a full cluster, including provisioning certs and deploying root certificates, you can use docker-compose via:

```
make run
```

Then, visit https://quic.video/publish/?server=localhost:4443.

# Usage

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

## moq-karp

[moq-karp](moq-karp) is a simple media layer on top of MoQ.
The crate includes a binary that accepts fMP4 with a few restrictions:

-   `separate_moof`: Each fragment must contain a single track.
-   `frag_keyframe`: A keyframe must be at the start of each keyframe.
-   `fragment_per_frame`: (optional) Each frame should be a separate fragment to minimize latency.

This can be used in conjunction with ffmpeg to publish media to a MoQ relay.
See [dev/pub](dev/pub) for the required ffmpeg flags.

Alternatively, see [moq-gst](https://github.com/kixelated/moq-gst) for a gstreamer plugin.
This is in a separate repository to avoid gstreamer being a hard requirement.

## moq-transfork

A media-agnostic library used by [moq-relay](moq-relay) and [moq-karp](moq-karp) to serve the underlying subscriptions.
It has caching/deduplication built-in, so your application is oblivious to the number of connections under the hood.

See the published [crate](https://crates.io/crates/moq-transfork) and [documentation](https://docs.rs/moq-transfork/latest/moq_transfork/).

## moq-clock

[moq-clock](moq-clock) is a simple client that can publish or subscribe to the current time.
It's meant to demonstate that [moq-transfork](moq-transfork) can be used for more than just media.


# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
