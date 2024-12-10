import * as Comlink from "comlink";
import { init } from "..";
import type * as Bridge from "../bridge";

export class Publish {
	#inner: Promise<Comlink.Remote<Bridge.Publish>>;

	constructor(src: string) {
		this.#inner = init.then((api) => api.publish(src));
	}

	async volume(value: number) {
		await (await this.#inner).volume(value);
	}

	async capture(media?: MediaStream) {
		const inner = await this.#inner;
		if (media === undefined) {
			return inner.capture();
		}

		return inner.capture(Comlink.transfer(media, [media]));
	}

	async close() {
		await (await this.#inner).close();
	}
}
