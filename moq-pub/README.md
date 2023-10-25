# moq-pub

A command line tool for publishing media via Media over QUIC (MoQ).

Expects to receive fragmented MP4 via standard input and connect to a MOQT relay.

```
ffmpeg ... - | moq-pub https://localhost:4443
```

### Invoking `moq-pub`:

Here's how I'm currently testing things, with a local copy of Big Buck Bunny named `bbb_source.mp4`:

```
$ ffmpeg -hide_banner -v quiet -stream_loop -1 -re -i bbb_source.mp4 -an -f mp4 -movflags empty_moov+frag_every_frame+separate_moof+omit_tfhd_offset - | RUST_LOG=moq_pub=info moq-pub https://localhost:4443
```

This relies on having `moq-relay` (the relay server) already running locally in another shell.

Note also that we're dropping the audio track (`-an`) above until audio playback is stabilized on the `moq-js` side.

### Known issues

-   Expects only one H.264/AVC1-encoded video track (catalog generation doesn't support audio tracks yet)
-   Doesn't yet gracefully handle EOF - workaround: never stop sending it media (`-stream_loop -1`)
-   Probably still full of lots of bugs
-   Various other TODOs you can find in the code
