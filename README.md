# Media over QUIC

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

```
./cert/generate
```

Unfortunately, WebTransport in Chrome currently (May 2023) doesn't verify certificates using the root CA.
The workaround is to use the `serverFingerprints` options, which requires the certificate MUST be only valid for at most **14 days**.
This is also why we're using a fork of mkcert, because it generates certificates valid for years by default.
This limitation will be removed once Chrome uses the system CA for WebTransport.

## Usage

Run the server:

```
cargo run
```

This listens for WebTransport connections on `https://localhost:4443` by default.
Use a [MoQ client](https://github.com/kixelated/moq-js) to connect to the server.

## License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
