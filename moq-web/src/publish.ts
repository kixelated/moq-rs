import * as Comlink from "comlink";
import type * as Bridge from "./bridge";
import { init } from ".";

export class Publish {
	#bridge: Promise<Comlink.Remote<Bridge.Publish>>;
	#video?: PublishVideo;

	constructor(src: string) {
		this.#bridge = init.then((api) => api.publish(src));
	}

	async capture(media?: MediaStream): Promise<PublishVideo | null> {
		if (!media) {
			// TODO tell the worker to end the current tracks
			return null;
		}

		const input = media.getVideoTracks().at(0);
		if (!input) {
			return null;
		}

		const bridge = await this.#bridge;
		const output = Comlink.proxy(await bridge.video(input.getSettings()));

		if (this.#video) {
			await this.#video.close();
		}

		this.#video = new PublishVideo(input, output);
		return this.#video;
	}

	async close() {
		await this.#video?.close();
		await (await this.#bridge).close();
	}
}

export class PublishVideo {
	#input: MediaStreamVideoTrack;
	#output: Bridge.PublishVideo & Comlink.ProxyMarked;
	#abort: AbortController;

	readonly closed: Promise<void>;

	constructor(input: MediaStreamVideoTrack, output: Bridge.PublishVideo & Comlink.ProxyMarked) {
		this.#input = input;
		this.#output = output;
		this.#abort = new AbortController();
		this.closed = this.#run().catch(console.error);
	}

	async #run() {
		const processor = new MediaStreamTrackProcessor({ track: this.#input });
		const reader = processor.readable.getReader();

		for (;;) {
			const next = await reader.read();
			if (next.done) {
				break;
			}

			await this.#output.encode(next.value);
		}
	}

	async close() {
		this.#abort.abort();
	}
}
