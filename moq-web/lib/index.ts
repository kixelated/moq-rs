import * as Comlink from "comlink";
import type * as Rust from "./worker";

const worker = new Worker(new URL("./worker", import.meta.url));

// Not 100% if this is the best solution, but Comlink was silently failing.
// We wait until the worker is fully initialized before we return the proxy.
const init: Promise<Comlink.Remote<Rust.Api>> = new Promise((resolve) => {
	worker.onmessage = (_event) => {
		worker.onmessage = null;
		const proxy: Comlink.Remote<Rust.Api> = Comlink.wrap(worker);
		resolve(proxy);
	};
});

export class Watch {
	#inner: Promise<Comlink.Remote<Rust.Watch>>;

	constructor(server: string, room: string, broadcast: string) {
		this.#inner = init.then((api) => api.watch(server, room, broadcast));
	}

	async pause(value: boolean) {
		await (await this.#inner).pause(value);
	}

	async volume(value: number) {
		await (await this.#inner).volume(value);
	}

	async render(value: HTMLCanvasElement | OffscreenCanvas) {
		const canvas =
			value instanceof HTMLCanvasElement
				? value.transferControlToOffscreen()
				: value;

		await (await this.#inner).render(Comlink.transfer(canvas, [canvas]));
	}

	async close() {
		await (await this.#inner).close();
	}

	async closed() {
		await (await this.#inner).closed();
	}
}
