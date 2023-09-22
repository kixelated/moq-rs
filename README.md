<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC streams.
See [quic.video](https://quic.video) for more information.

This repository contains a few crates:

-   **moq-relay**: A relay server, accepting content from publishers and fanning it out to subscribers.
-   **moq-pub**: A publish client, accepting media from stdin (ex. via ffmpeg) and sending it to a remote server.
-   **moq-transport**: An async implementation of the underlying MoQ protocol.

There's currently no way to actually view content with `moq-rs`; you'll need to use [moq-js](https://github.com/kixelated/moq-js) for that.

## Setup

You'll need [Rust](https://rustup.rs/) to build the project (duh).
The different crates have their own requirements.

### moq-relay

#### Certificate

Unfortunately, QUIC mandates TLS and makes local development difficult.
If you have a valid certificate you can use it instead of self-signing.

Use [mkcert](https://github.com/FiloSottile/mkcert) to generate a self-signed certificate.
Unfortunately, this currently requires [Go](https://golang.org/) to be installed in order to [fork](https://github.com/FiloSottile/mkcert/pull/513) the tool.
Somebody should get that merged or make something similar in Rust...

```bash
./dev/cert
```

Unfortunately, WebTransport in Chrome currently (May 2023) doesn't verify certificates using the root CA.
The workaround is to use the `serverFingerprints` options, which requires the certificate MUST be only valid for at most **14 days**.
This is also why we're using a fork of mkcert, because it generates certificates valid for years by default.
This limitation will be removed once Chrome uses the system CA for WebTransport.

#### Redis

`moq-relay` uses a redis instance to discover other relays and share broadcasts.
This is not relevant for development, but we still flex some of the code to make sure it works.

The `./dev/relay` script will automatically host a redis instance.
You will need either [Docker](https://www.docker.com/) or [Podman](https://podman.io/) installed.
Otherwise you can use the `--redis` flag to point to your own instance.

### moq-pub

#### Media

You'll want some test footage to broadcast.
Anything works, but make sure the codec is supported by the player since `moq-pub` does not re-encode.

Here's a criticially acclaimed short film:

```bash
mkdir media
wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O dev/source.mp4
```

#### ffmpeg

`moq-pub` uses [ffmpeg](https://ffmpeg.org/) to convert the media to fMP4.
You should have it installed already if you're a video nerd.

## Usage

### moq-relay

**moq-relay** is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
It's designed to be run in a datacenter, relaying media across multiple hops to deduplicate and improve QoS.
The relays register themselves in a redis instance, which is used to discover other relays and share broadcasts.

You can run the development server with the following command, automatically using the self-signed certificate generated earlier:

```bash
./dev/relay
```

Notable arguments:

-   `--bind <ADDR>` Listen on this address [default: [::]:4443]
-   `--cert <CERT>` Use the certificate file at this path
-   `--key <KEY>` Use the private key at this path

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

The server also listens on `TCP localhost:4443` when in development mode.
This is exclusively to serve a `/fingerprint` endpoint via HTTPS for self-signed certificates, which are not needed in production.

### moq-pub

This is a client that publishes a fMP4 stream from stdin over MoQ.
This can be combined with ffmpeg (and other tools) to produce a live stream.

The following command runs a development instance, broadcasing `dev/source.mp4` to `localhost:4443`:

```bash
./dev/pub
```

Notable arguments:

-   `<URI>` connect to the given address, which must start with moq://.

### moq-js

There's currently no way to consume broadcasts with `moq-rs`, at least until somebody writes `moq-sub`.
Until then, you can use [moq.js](https://github.com/kixelated/moq-js) both watch broadcasts and publish broadcasts.

There's a hosted version available at [quic.video](https://quic.video/).
There's a secret `?server` parameter that can be used to connect to a different address.

-   Publish to localhost: `https://quic.video/publish/?server=localhost:4443`
-   Watch from localhost: `https://quic.video/watch/<name>/?server=localhost:4443`

Note that self-signed certificates are ONLY supported if the server name starts with `localhost`.
You'll need to add an entry to `/etc/hosts` if you want to use a self-signed certs and an IP address.

## License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
