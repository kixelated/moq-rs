import type * as Comlink from "comlink";
import { init } from ".";
import type * as Bridge from "./bridge";

export class Publish {
	#inner: Promise<Comlink.Remote<Bridge.Publish>>;

	constructor(src: string) {
		this.#inner = init.then((api) => api.publish(src));
	}

	async media(media?: MediaStream) {
		if (!media) {
			// TODO tell the worker to end the current tracks
			return;
		}

		for (const track of media.getVideoTracks()) {
		}
	}

	async #runVideo(track: MediaStreamVideoTrack) {
		const inner = await this.#inner;

		const video = await inner.video(track.getSettings());
		const processor = new MediaStreamTrackProcessor({ track });
		const reader = processor.readable.getReader();

		for (;;) {
			const next = await reader.read();
			if (next.done) {
				break;
			}

			await video.encode(next.value);
		}
	}

	async close() {
		await (await this.#inner).close();
	}
}
