import * as Comlink from "comlink";
import { init } from ".";
import type * as Bridge from "./bridge";

export class Watch {
	#inner: Promise<Comlink.Remote<Bridge.Watch>>;

	constructor(src: string) {
		this.#inner = init.then((api) => api.watch(src));
	}

	async pause(value: boolean) {
		await (await this.#inner).pause(value);
	}

	async volume(value: number) {
		await (await this.#inner).volume(value);
	}

	async render(value: HTMLCanvasElement | OffscreenCanvas) {
		const canvas = value instanceof HTMLCanvasElement ? value.transferControlToOffscreen() : value;

		await (await this.#inner).render(Comlink.transfer(canvas, [canvas]));
	}

	async close() {
		await (await this.#inner).close();
	}
}
