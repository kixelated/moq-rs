import * as Comlink from "comlink";
import { init } from "./bridge";
import type * as Api from "./worker";

export class Watch {
	#bridge: Promise<Comlink.Remote<Api.Watch>>;

	constructor(src: string) {
		this.#bridge = init().then((api) => api.watch(src));
	}

	async pause(value: boolean) {
		await (await this.#bridge).pause(value);
	}

	async volume(value: number) {
		await (await this.#bridge).volume(value);
	}

	async render(value: HTMLCanvasElement | OffscreenCanvas) {
		const canvas = value instanceof HTMLCanvasElement ? value.transferControlToOffscreen() : value;

		await (await this.#bridge).render(Comlink.transfer(canvas, [canvas]));
	}

	async close() {
		await (await this.#bridge).close();
	}
}
