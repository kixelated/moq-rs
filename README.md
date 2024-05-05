<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC streams.
See [quic.video](https://quic.video) for more information.

This repository contains a few crates:

-   **moq-relay**: Accepting content from publishers and serves it to any subscribers.
-   **moq-pub**: Publishes fMP4 broadcasts.
-   **moq-transport**: An implementation of the underlying MoQ protocol.
-   **moq-api**: A HTTP API server that stores the origin for each broadcast, backed by redis.
-   **moq-dir**: Aggregates announcements, used to discover broadcasts.
-   **moq-clock**: A dumb clock client/server just to prove MoQ is more than media.

There's currently no way to view media with this repo; you'll need to use [moq-js](https://github.com/kixelated/moq-js) for that.
A hosted version is available at [quic.video](https://quic.video) and accepts the `?host=localhost:4443` query parameter.

# Development

Launch a basic cluster, including provisioning certs and deploying root certificates:

```
make run
```

Then, visit https://quic.video/publish/?server=localhost:4443.

For more control, use the [dev helper scripts](dev/README.md).

# Usage

## moq-relay

[moq-relay](moq-relay) is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
It's designed to be run in a datacenter, relaying media across multiple hops to deduplicate and improve QoS.
The relays optionally register themselves via the [moq-api](moq-api) endpoints, which is used to discover other relays and share broadcasts.

Notable arguments:

-   `--bind <ADDR>` Listen on this address, default: `[::]:4443`
-   `--tls-cert <CERT>` Use the certificate file at this path
-   `--tls-key <KEY>` Use the private key at this path
-   `--announce <URL>` Forward all announcements to this instance, typically [moq-dir](moq-dir).

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

## moq-pub

A client that publishes a fMP4 stream over MoQ, with a few restrictions.

-   `separate_moof`: Each fragment must contain a single track.
-   `frag_keyframe`: A keyframe must be at the start of each keyframe.
-   `fragment_per_frame`: (optional) Each frame should be a separate fragment to minimize latency.

This client can currently be used in conjuction with either ffmpeg or gstreamer.

### ffmpeg

moq-pub can be run as a binary, accepting a stream (from ffmpeg via stdin) and publishing it to the given relay.
See [dev/pub](dev/pub) for the required ffmpeg flags.

### gstreamer

moq-pub can also be run as a library, currently used for a [gstreamer plugin](https://github.com/kixelated/moq-gst).
This is in a separate repository to avoid gstreamer being a hard requirement.
See [run](https://github.com/kixelated/moq-gst/blob/main/run) for an example pipeline.

## moq-transport

A media-agnostic library used by [moq-relay](moq-relay) and [moq-pub](moq-pub) to serve the underlying subscriptions.
It has caching/deduplication built-in, so your application is oblivious to the number of connections under the hood.

See the published [crate](https://crates.io/crates/moq-transport) and [documentation](https://docs.rs/moq-transport/latest/moq_transport/).

## moq-clock

[moq-clock](moq-clock) is a simple client that can publish or subscribe to the current time.
It's meant to demonstate that [moq-transport](moq-transport) can be used for more than just media.

## moq-dir

[moq-dir](moq-dir) is a server that aggregates announcements.
It produces tracks based on the prefix, which are subscribable and can be used to discover broadcasts.

For example, if a client announces the broadcast `.public.room.12345.alice`, then `moq-dir` will produce the following track:

```
TRACK namespace=. track=public.room.12345.
OBJECT +alice
```

Use the `--announce <moq-dir-url>` flag when running the relay to forward all announcements to the instance.

## moq-api

This is a API server that exposes a REST API.
It's used by relays to inserts themselves as origins when publishing, and to find the origin when subscribing.
It's basically just a thin wrapper around redis that is only needed to run multiple relays in a (simple) cluster.

# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
