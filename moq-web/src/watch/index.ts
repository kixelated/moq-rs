import * as Comlink from "comlink";

// TODO investigate if enums are causing the WASM module to be loaded.
// Without enums, we could `import type` to avoid this?
import * as Rust from "@rust";
import { type ConnectionStatus, convertConnectionStatus } from "../connection";
import type { Bridge } from "./bridge";

export type RendererStatus = "idle" | "paused" | "buffering" | "live";

// Create a new worker instance that is shared between all instances of the Watch class.
// We wait until the worker is fully initialized before we return the proxy.
const worker: Promise<Comlink.Remote<Bridge>> = new Promise((resolve) => {
	const worker = new Worker(new URL("./bridge", import.meta.url), {
		type: "module",
	});
	worker.addEventListener(
		"message",
		(event) => {
			const proxy: Comlink.Remote<Bridge> = Comlink.wrap(worker);
			resolve(proxy);
		},
		{ once: true },
	);
});

export class Watch {
	#worker: Promise<Comlink.Remote<Rust.Watch>>;

	#url: string | null = null;
	#canvas: HTMLCanvasElement | null = null;
	#paused = false;
	#latency = 0;
	#volume = 1;

	constructor() {
		this.#worker = worker.then((w) => w.watch());
	}

	get url(): string | null {
		return this.#url;
	}

	set url(value: string | null) {
		this.#worker.then((worker) => worker.url(value));
		this.#url = value;
	}

	get canvas(): HTMLCanvasElement | null {
		return this.#canvas;
	}

	set canvas(value: HTMLCanvasElement | null) {
		const offscreen = value ? value.transferControlToOffscreen() : null;
		this.#worker.then((worker) => worker.canvas(Comlink.transfer(offscreen, offscreen ? [offscreen] : [])));
		this.#canvas = value;
	}

	get paused(): boolean {
		return this.#paused;
	}

	set paused(value: boolean) {
		this.#worker.then((worker) => worker.paused(value));
		this.#paused = value;
	}

	get latency(): number {
		return this.#latency;
	}

	// Set the target latency in ms.
	// A higher value means more stable playback.
	set latency(ms: number) {
		if (ms < 0) {
			throw new RangeError("latency must be greater than 0");
		}

		this.#worker.then((worker) => worker.latency(ms));
		this.#latency = ms;
	}

	get volume(): number {
		return this.#volume;
	}

	set volume(value: number) {
		if (value < 0 || value > 1) {
			throw new RangeError("volume must be between 0 and 1");
		}
		this.#worker.then((worker) => worker.volume(value));
		this.#volume = value;
	}

	async *connectionStatus(): AsyncGenerator<ConnectionStatus> {
		const worker = await this.#worker;
		const status = await Comlink.proxy(worker.status());

		for (;;) {
			const next = await status.connection();
			yield convertConnectionStatus(next);
		}
	}

	async *rendererStatus(): AsyncGenerator<RendererStatus> {
		const worker = await this.#worker;
		const status = await Comlink.proxy(worker.status());

		for (;;) {
			const next = await status.renderer();
			switch (next) {
				case Rust.RendererStatus.Idle:
					yield "idle";
					break;
				case Rust.RendererStatus.Paused:
					yield "paused";
					break;
				case Rust.RendererStatus.Buffering:
					yield "buffering";
					break;
				case Rust.RendererStatus.Live:
					yield "live";
					break;
				default: {
					const _exhaustive: never = next;
					throw new Error(_exhaustive);
				}
			}
		}
	}
}

export default Watch;
