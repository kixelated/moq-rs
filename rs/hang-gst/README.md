<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

A gstreamer plugin utilizing [moq](https://github.com/kixelated/moq).

# Usage
## Requirements
- [Rustup](https://www.rust-lang.org/tools/install)
- [Just](https://github.com/casey/just?tab=readme-ov-file#installation)

## Setup
We use `just` to simplify the development process.
Check out the [Justfile](justfile) or run `just` to see the available commands.

Install any other required tools:
```sh
just setup
```

## Development
First make sure you have a local moq-relay server running:
```sh
just relay
```

Now you can publish and subscribe to a video:
```sh
# Publish to a localhost moq-relay server
just pub-gst bbb

# Subscribe from a localhost moq-relay server
just sub bbb
```

# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
