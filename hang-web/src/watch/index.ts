import { Bridge } from "../bridge";

export class Watch {
	#bridge: Bridge;

	#latency = 0;

	constructor(bridge: Bridge) {
		this.#bridge = bridge;
	}

	get latency(): number {
		return this.#latency;
	}

	set latency(latency: number) {
		this.#bridge.postMessage({ Watch: { Latency: latency } });
		this.#latency = latency;
	}

	set canvas(canvas: HTMLCanvasElement | null) {
		const offscreen = canvas ? canvas.transferControlToOffscreen() : null;
		this.#bridge.postMessage({ Watch: { Canvas: offscreen } });
	}
}
