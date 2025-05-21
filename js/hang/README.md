<p align="center">
	<img height="128px" src="https://github.com/kixelated/moq/blob/main/.github/logo.svg" alt="Media over QUIC">
</p>

Media over QUIC (MoQ) is a live (media) delivery protocol utilizing QUIC.
It utilizes new browser technologies such as [WebTransport](https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API) and [WebCodecs](https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API) to provide WebRTC-like functionality.
Despite the focus on media, the transport is generic and designed to scale to enormous viewership via clustered relay servers (aka a CDN).
See [quic.video](https://quic.video) for more information.

**Note:** this project is a [fork](https://quic.video/blog/transfork) of the [IETF specification](https://datatracker.ietf.org/group/moq/documents/).
The principles are the same but the implementation is exponentially simpler given a narrower focus (and no politics).

# Usage
This library contains a lot of media stuff.
More documentation will be available later, until then refer to the code and especially the demos.

```html
    <!-- import "@kixelated/hang/publish/element" -->
	<hang-publish url="http://localhost:4443/demo/me.hang" audio video controls>
		<!-- It's optional to provide a video element to preview the outgoing media. -->
		<video style="max-width: 100%; height: auto; border-radius: 4px; margin: 0 auto;" muted autoplay></video>
	</hang-publish>

	<!-- import "@kixelated/hang/watch/element" -->
	<hang-watch url="http://localhost:4443/demo/me.hang" muted controls>
		<!-- It's optional to provide a canvas element to render the incoming media. -->
		<canvas style="max-width: 100%; height: auto; border-radius: 4px; margin: 0 auto;"></canvas>
	</hang-watch>
```

The API is still evolving, so expect breaking changes.

# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
