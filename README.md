<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC streams.
See [quic.video](https://quic.video) for more information.

This repository contains a few crates:

-   **moq-relay**: A relay server, accepting content from publishers and fanning it out to subscribers.
-   **moq-pub**: A publish client, accepting media from stdin (ex. via ffmpeg) and sending it to a remote server.
-   **moq-transport**: An async implementation of the underlying MoQ protocol.
-   **moq-api**: A HTTP API server that stores the origin for each broadcast, backed by redis.
-   **moq-clock**: A dumb clock client/server just to prove MoQ is more than media.

There's currently no way to view media with this repo; you'll need to use [moq-js](https://github.com/kixelated/moq-js) for that.

## Development

Launch a basic cluster, including provisioning certs and deploying root certificates:

```
make run
```

Then, visit https://quic.video/publish/?server=localhost:4443.

Alternatively, use the [dev helper scripts](dev/README.md).

## Usage

### moq-relay

**moq-relay** is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
It's designed to be run in a datacenter, relaying media across multiple hops to deduplicate and improve QoS.
The relays register themselves via the [moq-api](moq-api) endpoints, which is used to discover other relays and share broadcasts.

Notable arguments:

-   `--bind <ADDR>` Listen on this address, default: `[::]:4443`
-   `--tls-cert <CERT>` Use the certificate file at this path
-   `--tls-key <KEY>` Use the private key at this path
-   `--dev` Listen via HTTPS as well, serving the `/fingerprint` of the self-signed certificate. (dev only)

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

### moq-pub

This is a client that publishes a fMP4 stream from stdin over MoQ.
This can be combined with ffmpeg (and other tools) to produce a live stream.

Notable arguments:

-   `<URL>` connect to the given address, which must start with `https://` for WebTransport.

**NOTE**: We're very particular about the fMP4 ingested. See [this script](dev/pub) for the required ffmpeg flags.

### moq-transport

A media-agnostic library used by [moq-relay](moq-relay) and [moq-pub](moq-pub) to serve the underlying subscriptions.
It has caching/deduplication built-in, so your application is oblivious to the number of connections under the hood.

See the published [crate](https://crates.io/crates/moq-transport) and [documentation](https://docs.rs/moq-transport/latest/moq_transport/).

### moq-api

This is a API server that exposes a REST API.
It's used by relays to inserts themselves as origins when publishing, and to find the origin when subscribing.
It's basically just a thin wrapper around redis that is only needed to run multiple relays in a (simple) cluster.

## License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
