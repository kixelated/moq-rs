# moq-pub

WARNING: Work in Progress!!!

This is currently a *very* messy branch with a lot of debugging going on and also isn't currently factored in the way I eventualy imagine implementing this tool. 

_However_, it is getting close to being in a working state. Once I have a version that works end-to-end, I'll start cleaning up and refactoring towards a more elegant design, probably on a new branch.

In the meantime, here are some things that may be worth noting about the current WIP code I have here:

### Current `moq-pub` components

- `Media` is responsible for reading from stdin and parsing MP4 boxes. It populates a `MapSource` of `Track`s for which it holds the producer side, pushing segments of video/audio into them and notifying consumers via tokio watch async primitives.

- `SessionRunner` is currrently where we create and hold the MOQT Session from the moq_transport_quinn library, and we use a series of `mpsc` and `broadcast` channels to make it possible for other parts of our code to send/recieve control messages and objects (data) via that Session. At present, evenything that comes from the wire or goes onto the wire will route through here (in `session_runner.run()`)

- `MediaRunner` will be responsible for consuming the `Track`s that `Media` produces and populates. `MediaRunner` will spawn tasks for each `Track` to `.await` new segments and then put the media data into Objects and onto the wire (via channels into `SessionRunner`)

- `LogViewer` will snoop on some channels going in/out of `SessionRunner` and be responsible for exposing useful debugging/troubleshooting information. Currently we just have a bunch of `dbg!()` lines scattered about in `SessionRunner` itself (and elsewhere). When we start cleaning those up, this is where some better logging functionality will reside.

Longer term, I'd like to refactor everything such that the `Media` + `MediaRunner` bits will consume an interface that's _closer_ to what we'd like to eventually expose as a C FFI for consumption by external tools. That probably means greatly reducing the use of async Rust in the parts of this code that make up both sides of that interface boundary.


### Invoking `moq-pub`:

Here's how I'm currently testing things, with a local copy of Big Buck Bunny named `bbb_source.mp4`:

```
$ RUST_LOG=info ffmpeg -hide_banner -v quiet -stream_loop 0 -re -i ../media/bbb_source.mp4 -f mp4 -movflags empty_moov+frag_every_frame+separate_moof+omit_tfhd_offset - | cargo run
```

This relies on having `moq-demo` (the relay server) already running locally in another shell.
