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
This library contains just the generic transport.
More documentation will be available later, until then refer to the code.

```ts
import * as Moq from "@kixelated/moq";

const conn = await Moq.Connection.connect(new URL("http://localhost:4443/"));

// Optional: Discover broadcasts matching a prefix.
const announced = conn.announced("demo/");
for (;;) {
	const announce = await announced.next();
	if (!announce) break;

	console.log("discovered broadcast:", announce.path);

	// NOTE: This code is untested and the API is subject to change.
	const broadcast = conn.consume(`demo/${announce.path}`);
	const track = broadcast.subscribe("catalog.json");

	// A track can have multiple groups (unordered+unreliable).
	const group = await track.nextGroup();

	// A group can have multiple frames (ordered+reliable).
	const frame = await group.nextFrame();

	// This is a Uint8Array; but the contents are actually JSON.
	console.log("received frame:", frame);

	// Don't forget to close to release resources.
	frame.close();
	group.close();
	track.close();
	broadcast.close();
}
```


# License

Licensed under either:

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
