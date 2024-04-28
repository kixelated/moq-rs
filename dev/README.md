# Local Development

## Quickstart with Docker

Launch a basic cluster, including provisioning certs and deploying root certificates:

```
# From repo root:
make run
```

Then, visit https://quic.video/publish/?server=localhost:4443.

## Manual setup

This is a collection of helpful scripts for local development.

### moq-relay

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

### moq-pub

You'll want some test footage to broadcast.
Anything works, but make sure the codec is supported by the player since `moq-pub` does not re-encode.

Here's a criticially acclaimed short film:

```bash
mkdir media
wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O dev/source.mp4
```

`moq-pub` uses [ffmpeg](https://ffmpeg.org/) to convert the media to fMP4.
You should have it installed already if you're a video nerd, otherwise:

```bash
brew install ffmpeg
```

### moq-api

`moq-api` uses a redis instance to store active origins for clustering.
This is not relevant for most local development and the code path is skipped by default.

However, if you want to test the clustering, you'll need either either [Docker](https://www.docker.com/) or [Podman](https://podman.io/) installed.
We run the redis instance via a container automatically as part of `dev/api`.

## Development

**tl;dr** run these commands in seperate terminals:

```bash
./dev/cert
./dev/relay
./dev/pub
```

They will each print out a URL you can use to publish/watch broadcasts.

### moq-relay

You can run the relay with the following command, automatically using the self-signed certificates generated earlier.
This listens for WebTransport connections on WebTransport `https://localhost:4443` by default.

```bash
./dev/relay
```

It will print out a URL when you can use to publish. Alternatively, you can use `dev/pub` instead.

> Publish URL: https://quic.video/publish/?server=localhost:4443

### moq-pub

The following command runs a development instance, broadcasing `dev/source.mp4` to WebTransport `https://localhost:4443`:

```bash
./dev/pub
```

It will print out a URL when you can use to watch.
By default, the broadcast name is `dev` but you can overwrite it with the `NAME` env.

> Watch URL: https://quic.video/watch/dev?server=localhost:4443

### moq-api

The following commands runs an API server, listening for HTTP requests on `http://localhost:4442` by default.

```bash
./dev/api
```
