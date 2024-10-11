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

## moq-sub

You can use `moq-sub` to subscribe to media streams from a MoQ relay and pipe them to the standard output.
By piping the command to a video player, e.g. `ffplay` or `mpv`, you can play a MoQ broadcast natively.

Currently, `moq-sub` simply dumps all received segments of the first video and the first audio track
directly to `stdout`.

The following command subscribes to a stream from a MoQ relay and plays it with `ffplay`.
By default, the URL is `https://localhost:4433/dev`, so it will play the stream published with `dev/pub`
to the relay started with `dev/relay`. You can change the broadcast name by setting the `NAME` env var.

```bash
./dev/sub
```

## moq-clock

To show that MoQ can do more than just media, we made a simple clock.

```bash
./dev/clock --publish
```

And run the subscriber in a separate terminal:

```bash
./dev/clock
```
