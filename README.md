<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC streams.
See [quic.video](https://quic.video) for more information.

This repository contains a few crates:

-   **moq-relay**: Forwards content from publishers to any interested subscribers.
-   **moq-karp**: A simple media layer and CLI, powered by moq-transfork.
-   **moq-transfork**: The implementation of the underlying MoQ protocol.
-   **moq-clock**: A dumb clock client/server just to prove MoQ is more than media.
-   **moq-native**: Helpers to configure MoQ CLIs.

There's currently no way to view media with this repo; you'll need to use [moq-js](https://github.com/kixelated/moq-js) for that.
A hosted version is available at [quic.video](https://quic.video) and accepts the `?host=localhost:4443` query parameter.

# Development
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
