import { Bridge } from "../bridge";

export class Watch {
	#bridge: Bridge;

	#url: URL | null = null;
	#latency = 0;
	#visible = true;

	#volume = 1;
	#gain: GainNode | null = null;

	constructor(bridge: Bridge) {
		this.#bridge = bridge;
	}

	// If a null canvas is provided, video rendering will be disabled.
	initVideo(canvas: HTMLCanvasElement | null) {
		this.#bridge.postMessage({ Watch: { Canvas: canvas?.transferControlToOffscreen() ?? null } });
	}

	// Initialize audio rendering.
	//
	// Must be called on the main thread.
	// Must be called after the user interacts with the page.
	initAudio() {
		const context = new AudioContext({ latencyHint: "interactive" });
		const gain = context.createGain();
		gain.gain.value = this.#volume;

		gain.connect(context.destination);

		// NOTE: rspack works by matching against the `context` variable name, so we can't change it.
		context.audioWorklet.addModule(new URL("../worklet", import.meta.url)).then(() => {
			const worklet = new AudioWorkletNode(context, "renderer");
			worklet.connect(gain);

			// Give Rust the port so it can send audio data to the worklet.
			// Rust is also responsible for resampling the audio to the correct sample rate.
			this.#bridge.postMessage({
				Watch: {
					Worklet: {
						port: worklet.port,
						sample_rate: context.sampleRate,
					},
				},
			});
		});

		this.#gain = gain;
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

	get visible(): boolean {
		return this.#visible;
	}

	set visible(visible: boolean) {
		this.#visible = visible;
		this.#bridge.postMessage({ Watch: { Visible: visible } });
	}

	get volume(): number {
		return this.#volume;
	}

	set volume(volume: number) {
		this.#volume = volume;

		if (this.#gain) {
			this.#gain.gain.value = volume;
		}

		if (this.#volume === 0) {
			this.#bridge.postMessage({ Watch: { Muted: true } });
		} else {
			this.#bridge.postMessage({ Watch: { Muted: false } });
		}
	}
}

// A custom element making it easier to insert into the DOM.
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url", "volume"];

	#bridge = new Bridge();
	#watch = new Watch(this.#bridge);

	// Detect if the canvas is hidden.
	#intersection: IntersectionObserver;
	#canvas: HTMLCanvasElement | null = null;

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

		this.addEventListener("click", () => this.#watch.initAudio(), { once: true });
	}

	#setCanvas(canvas: HTMLCanvasElement | null) {
		if (this.#canvas) {
			this.#intersection.unobserve(this.#canvas);
		}

		if (canvas) {
			this.#intersection.observe(canvas);
		}

		this.#canvas = canvas;
		this.#watch.initVideo(this.#canvas);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.#watch.url = newValue ? new URL(newValue) : null;
		} else if (name === "volume") {
			this.#watch.volume = Number.parseFloat(newValue ?? "1");
		}
	}
}

customElements.define("hang-watch", WatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": WatchElement;
	}
}
