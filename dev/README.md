# Local Development

This is a collection of helpful scripts for local development.
Check the source for each script for a list of useful arguments.

## moq-relay

Hosts a [relay server](../moq-relay) on `localhost:4443` using self-signed certificates.

Requires:

-   Rust
-   Go (mkcert)

```bash
./dev/relay
```

All clients listed can connect to this relay instance to publish and/or subscribe.
You can do this via [moq-js](https://github.com/kixelated/moq-js) for a UI, either self-hosted or accessed via https://quic.video/publish/?server=localhost:4443.

The purpose of a relay is to support clustering and fanout.
Like mentioned in the root README, the easiest way to do this is via docker compose:

```bash
make run
```

This hosts a Redis instance and [moq-api](../moq-api) instance to store the list of origins.
It also hosts a [moq-dir](../moq-dir) instance to serve the current announcements.

## moq-pub

Publish some test footage from disk to the localhost relay using [moq-pub](../moq-pub).
This downloads Big Buck Bunny and publishes a broadcast named `bbb`.

Requires:

-   Rust
-   ffmpeg

```bash
./dev/pub
```

Alternatively, you can use Gstreamer via [moq-gst](https://github.com/kixelated/moq-gst).
The [run](https://github.com/kixelated/moq-gst/blob/main/run) script does the exact same thing.

## moq-clock

To show that MoQ can do more than just media, we made a simple clock.

```bash
./dev/clock --publish
```

And run the subscriber in a separate terminal:

```bash
./dev/clock
```
