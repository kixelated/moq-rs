import * as Comlink from "comlink";
import type { Bridge } from "./bridge";

import type * as Rust from "@dist/rust";

export type { WatchState };
type WatchState = Rust.WatchState;

let cached: Promise<Comlink.Remote<Bridge>> | null = null;

// Create a new worker instance that is shared between all instances of the Watch class.
// We wait until the worker is fully initialized before we return the proxy.
function init(): Promise<Comlink.Remote<Bridge>> {
	if (!cached) {
		cached = new Promise((resolve) => {
			const worker = new Worker(new URL("./bridge", import.meta.url), { type: "module" });
			worker.addEventListener(
				"message",
				(event) => {
					const proxy: Comlink.Remote<Bridge> = Comlink.wrap(worker);
					resolve(proxy);
				},
				{ once: true },
			);
		});
	}

	return cached;
}

export class Watch {
	#watch: Promise<Comlink.Remote<Rust.Watch>>;

	#url: string | null = null;
	#volume = 1;
	#paused = false;
	#canvas: OffscreenCanvas | null = null;
	#error: string | null = null;

	constructor() {
		this.#watch = init().then((api) => api.watch());
	}

	get url(): string | null {
		return null;
	}

	set url(value: string | null) {
		this.#watch.then((watch) => watch.url(value));
	}

	get paused(): boolean {
		return this.#paused;
	}

	set paused(value: boolean) {
		this.#paused = value;
		this.#watch.then((watch) => watch.paused(value));
	}

	get volume(): number {
		return this.#volume;
	}

	set volume(value: number) {
		if (value < 0 || value > 1) {
			throw new RangeError("volume must be between 0 and 1");
		}
		this.#volume = value;
		this.#watch.then((watch) => watch.volume(value));
	}

	get canvas(): OffscreenCanvas | null {
		return this.#canvas;
	}

	set canvas(value: HTMLCanvasElement | OffscreenCanvas | null) {
		const canvas = value instanceof HTMLCanvasElement ? value.transferControlToOffscreen() : value;
		this.#canvas = canvas;
		this.#watch.then((watch) => watch.canvas(canvas ? Comlink.transfer(canvas, [canvas]) : null));
	}

	async close() {
		this.#watch.then((watch) => watch.close());
	}

	async *state(): AsyncGenerator<WatchState> {
		const watch = await this.#watch;
		const states = await Comlink.proxy(watch.states());

		while (true) {
			const next = await states.next();
			if (!next) {
				return;
			}

			yield next;
		}
	}

	get error(): string | null {
		// TODO implement
		return this.#error;
	}
}
