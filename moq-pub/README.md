# moq-pub

A command line tool for publishing media via Media over QUIC (MoQ).

Expects to receive fragmented MP4 via standard input and connect to a MOQT relay.

```
ffmpeg ... - | moq-pub -i - -u https://localhost:4443
```

### A note on the `moq-pub` code organization

- `Media` is responsible for reading from stdin and parsing MP4 boxes. It populates a `MapSource` of `Track`s for which it holds the producer side, pushing segments of video/audio into them and notifying consumers via tokio watch async primitives.

- `SessionRunner` is where we create and hold the MOQT Session from the `moq_transport` library. We currently hard-code our implementation to use `quinn` as the underlying WebTranport implementation. We use a series of `mpsc` and `broadcast` channels to make it possible for other parts of our code to send/recieve control messages via that Session. Sending Objects is handled a little differently because we are able to clone the MOQT Session's sender wherever we need to do that.

- `MediaRunner` is responsible for consuming the `Track`s that `Media` produces and populates. `MediaRunner` spawns tasks for each `Track` to `.await` new segments and then put the media data into Objects and onto the wire (via channels into `SessionRunner`). Note that these tasks are created, but block waiting un the reception of a MOQT SUBSCRIBE message before they actually send any segments on the wire. `MediaRunner` is also responsible for sending the initial MOQT ANNOUNCE message announcing the namespace for the tracks we will send.

- `LogViewer` as the name implies is responsible for logging. It snoops on some channels going in/out of `SessionRunner` and logs MOQT control messages.

Longer term, I think it'd be interesting to refactor everything such that the `Media` + `MediaRunner` bits consume an interface that's _closer_ to what we'd like to eventually expose as a C FFI for consumption by external tools. That probably means greatly reducing the use of async Rust in the parts of this code that make up both sides of that interface boundary.


### Invoking `moq-pub`:

Here's how I'm currently testing things, with a local copy of Big Buck Bunny named `bbb_source.mp4`:

```
$ ffmpeg -hide_banner -v quiet -stream_loop -1 -re -i bbb_source.mp4 -an -f mp4 -movflags empty_moov+frag_every_frame+separate_moof+omit_tfhd_offset - | RUST_LOG=moq_pub=info moq-pub -i -
```

This relies on having `moq-quinn` (the relay server) already running locally in another shell.

Note also that we're dropping the audio track (`-an`) above until audio playback is stabilized on the `moq-js` side.

### Known issues

- Expects only one H.264/AVC1-encoded video track (catalog generation doesn't support audio tracks yet)
- Doesn't yet gracefully handle EOF - workaround: never stop sending it media (`-stream_loop -1`)
- Probably still full of lots of bugs
- Various other TODOs you can find in the code
