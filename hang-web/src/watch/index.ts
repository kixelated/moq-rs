import { Bridge } from "../bridge";

export class Watch {
	#bridge: Bridge;

	#url: URL | null = null;
	#latency = 0;

	constructor(bridge: Bridge) {
		this.#bridge = bridge;
	}

	get url(): URL | null {
		return this.#url;
	}

	set url(url: URL | null) {
		this.#url = url;
		this.#bridge.postMessage({ Watch: { Connect: url?.toString() ?? null } });
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

// A custom element making it easier to insert into the DOM.
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url"];

	#bridge = new Bridge();
	#watch = new Watch(this.#bridge);

	constructor() {
		super();

		const canvas = document.createElement("canvas");

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => this.#updateCanvas(slot));
		slot.appendChild(canvas);

		this.attachShadow({ mode: "open" }).appendChild(slot);
	}

	#updateCanvas(slot: HTMLSlotElement) {
		for (const el of slot.assignedElements({ flatten: true })) {
			if (el instanceof HTMLCanvasElement) {
				this.#watch.canvas = el;
				return;
			}
		}

		this.#watch.canvas = null;
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.#watch.url = newValue ? new URL(newValue) : null;
		}
	}
}

customElements.define("hang-watch", WatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": WatchElement;
	}
}
