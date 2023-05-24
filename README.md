# Media over QUIC

Media over QUIC (MoQ) is a live media delivery protocol utilizing QUIC streams.
See the [Warp draft](https://datatracker.ietf.org/doc/draft-lcurley-warp/).

This repository is a Rust server that supports both contribution (ingest) and distribution (playback).
It requires a client, such as [moq-js](https://github.com/kixelated/moq-js).

## Requirements

-   _Chrome_: currently (May 2023) the only browser to support both WebTransport and WebCodecs.
-   _yarn_: required to install dependencies.

## Requirements

-   _rust_: duh
-   _ffmpeg_: (optional) used to generate fragmented media
-   _go_: (optional) used to generate self-signed certificates
-   _openssl_: (options) ...also used to generate self-signed certificates

## Media

This demo simulates a live stream by reading a file from disk and sleeping based on media timestamps. Obviously you should hook this up to a real live stream to do anything useful.

Download your favorite media file and convert it to fragmented MP4, by default `media/fragmented.mp4`:

```
wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O media/source.mp4
./media/generate
```

## Certificates

Unfortunately, QUIC mandates TLS and makes local development difficult.
If you have a valid certificate you can use it instead of self-signing.

Otherwise, we use [mkcert](https://github.com/FiloSottile/mkcert) to install a self-signed CA:

```
./cert/generate
```

With no arguments, the server will generate self-signed cert using this root CA.
This certificate is only valid for _2 weeks_ due to how WebTransport performs certificate fingerprinting.

## Server

`The Warp server supports WebTransport, pushing media over streams once a connection has been established.

```
cargo run
```

This listens for WebTransport connections on `https://localhost:4443` by default.
Use a [MoQ client](https://github.com/kixelated/moq-js) to connect to the server.
