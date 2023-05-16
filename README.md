# Warp
Live media delivery protocol utilizing QUIC streams. See the [Warp draft](https://datatracker.ietf.org/doc/draft-lcurley-warp/).

Warp works by delivering media over independent QUIC stream. These streams are assigned a priority such that old video will arrive last and can be dropped. This avoids buffering in many cases, offering the viewer a potentially better experience.

This demo requires WebTransport and WebCodecs, which currently (May 2023) only works on Chrome.

# Development
## Easy Mode
Requires Docker *only*.

```
docker-compose up --build
```

Then open [https://localhost:4444/](https://localhost:4444) in a browser. You'll have to click past the TLS error, but that's the price you pay for being lazy. Follow the more in-depth instructions if you want a better development experience.

## Requirements
* Go
* Rust
* ffmpeg
* openssl
* Chrome

## Media
This demo simulates a live stream by reading a file from disk and sleeping based on media timestamps. Obviously you should hook this up to a real live stream to do anything useful.

Download your favorite media file and convert it to fragmented MP4:
```
wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O media/source.mp4
./media/fragment
```

## Certificates
Unfortunately, QUIC mandates TLS and makes local development difficult.
If you have a valid certificate you can use it instead of self-signing.

Otherwise, we use [mkcert](https://github.com/FiloSottile/mkcert) to install a self-signed CA:
```
./generate/cert
```

With no arguments, the server will generate self-signed cert using this root CA. This certificate is only valid for *2 weeks* due to how WebTransport performs certificate fingerprinting.

## Server
The Warp server supports WebTransport, pushing media over streams once a connection has been established. A more refined implementation would load content based on the WebTransport URL or some other messaging scheme.

```
cd server
cargo run
```

This listens for WebTransport connections (not HTTP) on `https://localhost:4443` by default.

## Web
The web assets need to be hosted with a HTTPS server.

```
cd web
yarn install
yarn serve
```

These can be accessed on `https://localhost:4444` by default.