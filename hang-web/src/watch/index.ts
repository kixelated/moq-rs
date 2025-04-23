import { Bridge } from "../bridge";
import { ConnectionStatus, Event } from "../message";

export interface WatchEvents {
	connection: ConnectionStatus;
}

export class Watch {
	#bridge: Bridge = new Bridge();

	#url: URL | null = null;
	#latency = 0;
	#visible = true;
	#paused = false;

	#volume = 1;
	#gain: GainNode | null = null;

	#events = new EventTarget();

	constructor() {
		this.#bridge.addEventListener((event: Event) => {
			if (typeof event === "object" && event.Connection) {
				this.#dispatchEvent("connection", event.Connection);
			}
		});
	}

	addEventListener<K extends keyof WatchEvents>(type: K, listener: (event: CustomEvent<WatchEvents[K]>) => void) {
		this.#events.addEventListener(type, listener as EventListener);
	}

	removeEventListener<K extends keyof WatchEvents>(type: K, listener: (event: CustomEvent<WatchEvents[K]>) => void) {
		this.#events.removeEventListener(type, listener as EventListener);
	}

	#dispatchEvent<K extends keyof WatchEvents>(type: K, detail: WatchEvents[K]) {
		this.#events.dispatchEvent(new CustomEvent(type, { detail }));
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

	get paused(): boolean {
		return this.#paused;
	}

	set paused(paused: boolean) {
		this.#paused = paused;
		this.#bridge.postMessage({ Watch: { Paused: paused } });
	}
}

// A custom element making it easier to insert into the DOM.
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url", "volume", "latency"];

	// Expose the library so we don't have to duplicate everything.
	readonly lib = new Watch();

	// Detect if the canvas is hidden.
	#intersection: IntersectionObserver;
	#canvas: HTMLCanvasElement | null = null;

	constructor() {
		super();

		this.#intersection = new IntersectionObserver((entries) => {
			for (const entry of entries) {
				this.lib.visible = entry.isIntersecting;
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
			this.lib.visible = document.visibilityState === "visible";
		});

		this.attachShadow({ mode: "open" }).appendChild(slot);

		this.addEventListener("click", () => this.lib.initAudio(), { once: true });

		// Proxy the watch events to the element.
		this.lib.addEventListener("connection", (event) => {
			this.dispatchEvent(new CustomEvent("hang-connection", { detail: event.detail }));
		});
	}

	#setCanvas(canvas: HTMLCanvasElement | null) {
		if (this.#canvas) {
			this.#intersection.unobserve(this.#canvas);
		}

		if (canvas) {
			this.#intersection.observe(canvas);
		}

		this.#canvas = canvas;
		this.lib.initVideo(this.#canvas);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.url = newValue ? new URL(newValue) : null;
		} else if (name === "volume") {
			this.lib.volume = Number.parseFloat(newValue ?? "1");
		} else if (name === "latency") {
			this.lib.latency = Number.parseInt(newValue ?? "0");
		}
	}
}

customElements.define("hang-watch", WatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": WatchElement;
	}
}

declare global {
	interface HTMLElementEventMap {
		"hang-connection": CustomEvent<ConnectionStatus>;
	}
}
