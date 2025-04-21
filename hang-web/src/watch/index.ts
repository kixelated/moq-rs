import { Bridge } from "../bridge";

export class Watch {
	#bridge: Bridge;

	#url: URL | null = null;
	#latency = 0;
	#canvas: HTMLCanvasElement | null = null;
	#visible = true;

	#context: AudioContext;
	#volume: GainNode;

	constructor(bridge: Bridge) {
		this.#bridge = bridge;
		this.#context = new AudioContext();
		this.#volume = this.#context.createGain();
		this.#volume.connect(this.#context.destination);

		this.#context.audioWorklet.addModule(new URL("./worklet", import.meta.url)).then(() => {
			const worklet = new AudioWorkletNode(this.#context, "renderer");
			worklet.connect(this.#volume);

			// Give Rust the port so it can send audio data to the worklet.
			this.#bridge.postMessage({ Watch: { Worklet: worklet.port } });
		});
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

	get canvas(): HTMLCanvasElement | null {
		return this.#canvas;
	}

	set canvas(canvas: HTMLCanvasElement | null) {
		this.#canvas = canvas;
		this.#bridge.postMessage({ Watch: { Canvas: canvas?.transferControlToOffscreen() ?? null } });
	}

	get visible(): boolean {
		return this.#visible;
	}

	set visible(visible: boolean) {
		this.#visible = visible;
		this.#bridge.postMessage({ Watch: { Visible: visible } });
	}
}

// A custom element making it easier to insert into the DOM.
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url"];

	#bridge = new Bridge();
	#watch = new Watch(this.#bridge);

	// Detect if the canvas is hidden.
	#intersection: IntersectionObserver;

	constructor() {
		super();

		this.#intersection = new IntersectionObserver((entries) => {
			for (const entry of entries) {
				this.#watch.visible = entry.isIntersecting;
			}
		});

		const canvas = document.createElement("canvas");
		this.#setCanvas(canvas);

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLCanvasElement) {
					this.#setCanvas(el);
					return;
				}
			}

			this.#setCanvas(null);
		});
		slot.appendChild(canvas);

		// TODO Implement this properly so it doesn't fight with the intersection observer.
		document.addEventListener("visibilitychange", () => {
			this.#watch.visible = document.visibilityState === "visible";
		});

		this.attachShadow({ mode: "open" }).appendChild(slot);
	}

	#setCanvas(canvas: HTMLCanvasElement | null) {
		if (this.#watch.canvas) {
			this.#intersection.unobserve(this.#watch.canvas);
		}

		if (canvas) {
			this.#watch.canvas = canvas;
			this.#intersection.observe(canvas);
		} else {
			this.#watch.canvas = null;
		}
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
