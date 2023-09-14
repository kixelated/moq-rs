# Media over QUIC

<p align="center">
	<img height="256" src="https://github.com/kixelated/moq-rs/blob/main/.github/logo.svg">
</p>

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC streams.
See the [MoQ working group](https://datatracker.ietf.org/wg/moq/about/) for more information.

This repository contains reusable libraries and a relay server.
It requires a client to actually publish/view content, such as [moq-js](https://github.com/kixelated/moq-js).

Join the [Discord](https://discord.gg/FCYF3p99mr) for updates and discussion.

## Setup

### Certificates

Unfortunately, QUIC mandates TLS and makes local development difficult.
If you have a valid certificate you can use it instead of self-signing.

Use [mkcert](https://github.com/FiloSottile/mkcert) to generate a self-signed certificate.
Unfortunately, this currently requires Go in order to [fork](https://github.com/FiloSottile/mkcert/pull/513) the tool.

```bash
./dev/cert
```

Unfortunately, WebTransport in Chrome currently (May 2023) doesn't verify certificates using the root CA.
The workaround is to use the `serverFingerprints` options, which requires the certificate MUST be only valid for at most **14 days**.
This is also why we're using a fork of mkcert, because it generates certificates valid for years by default.
This limitation will be removed once Chrome uses the system CA for WebTransport.

### Media

If you're using `moq-pub` then you'll want some test footage to broadcast.

```bash
mkdir media
wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O dev/source.mp4
```

## Usage

### moq-relay

**moq-relay** is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
You can run the development server with:

```bash
./dev/relay
```

Notable flags:

-   `-a <ADDR>` Listen on this address [default: [::]:4443]
-   `-c <CERT>` Use the certificate file at this path [default: dev/localhost.crt]
-   `-k <KEY>` Use the private key at this path [default: dev/localhost.key]

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

The server also listens on `TCP localhost:4443` when in development mode.
This is exclusively to serve a `/fingerprint` endpoint via HTTPS for self-signed certificates, which are not needed in production.

### moq-pub

This is a client that publishes a fMP4 file from stdin over MoQ.
This can be combined with ffmpeg to produce a live stream from a file on disk, which is great for testing.
You can use this command to broadcast `dev/source.mp4` to `localhost:4443` to test the relay:

```bash
./dev/pub
```

Notable flags:

-   `--host <HOST>` connect to the given host [default: localhost:4443]
-   `--name <NAME>` produce a broadcast with the given name [default: random-uuid]
-   `--input <INPUT>` read from the input file, or `-` for stdin.

### moq-js

There's currently no way to consume broadcasts with `moq-rs`, at least until somebody writes `moq-sub`.
Until then, you can use [moq.js](https://github.com/kixelated/moq-js) both watch broadcasts and publish broadcasts.

There's a hosted version available at [quic.video](https://quic.video/).
There's a secret `?server` parameter that can be used to connect to a different address.

-   Publish to localhost: `https://quic.video/publish/?server=localhost:4443`
-   Watch from localhost: `https://quic.video/watch/<namespace>/?server=localhost:4443`

Note that self-signed certificates are ONLY supported if the server name starts with `localhost`.
You'll need to add an entry to `/etc/hosts` if you want to use a self-signed certs and an IP address.

## License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
