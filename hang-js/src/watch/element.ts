import * as Moq from "@kixelated/moq";

import { AudioEmitter, AudioSource } from "./audio";
import { BroadcastReload } from "./broadcast";
import { VideoRenderer, VideoSource } from "./video";

// A custom element that renders to a canvas.
export class Element extends HTMLElement {
	static observedAttributes = ["url", "name", "paused", "muted", "latency"];

	#url?: URL;
	#name?: string;
	#paused = false;
	#muted = false;
	#volume = 0.5;

	// TODO this is pretty high because the BBB audio stutters; fix that.
	#latency = 100;

	#connection?: Moq.ConnectionReload;

	#broadcast?: BroadcastReload;

	#audio?: AudioSource;
	#video?: VideoSource;

	#emitter = new AudioEmitter();
	#renderer = new VideoRenderer();

	constructor() {
		super();

		const canvas = document.createElement("canvas");
		canvas.style.width = "100%";
		canvas.style.height = "auto";

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLCanvasElement) {
					this.#renderer.canvas = el;
					return;
				}
			}

			this.#renderer.canvas = undefined;
		});

		// TODO: Move this to AudioSource?
		this.#emitter.latency = this.#latency;

		slot.appendChild(canvas);
		this.#renderer.canvas = canvas;

		const style = document.createElement("style");
		style.textContent = `
			:host {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		`;

		this.attachShadow({ mode: "open" }).append(style, slot);
	}

	attributeChangedCallback(name: string, oldValue: string | undefined, newValue: string | undefined) {
		if (oldValue === newValue) {
			return;
		}

		if (name === "url") {
			this.url = newValue ? new URL(newValue) : undefined;
		} else if (name === "name") {
			this.name = newValue;
		} else if (name === "paused") {
			this.paused = newValue !== undefined;
		} else if (name === "muted") {
			this.muted = newValue !== undefined;
		}
	}

	get url() {
		return this.#url;
	}

	set url(url: URL | undefined) {
		this.#url = url;

		this.#connection?.close();
		this.#connection = undefined;

		if (!url) {
			return;
		}

		this.#connection = new Moq.ConnectionReload(url);

		this.#connection.on("connecting", () => {
			this.dispatchEvent(new CustomEvent("moq-connection", { detail: "connecting" }));
		});

		this.#connection.on("connected", (_connection) => {
			this.dispatchEvent(new CustomEvent("moq-connection", { detail: "connected" }));
			this.#run();
		});

		this.#connection.on("disconnected", () => {
			this.dispatchEvent(new CustomEvent("moq-connection", { detail: "disconnected" }));
			this.#stop();
		});
	}

	get name() {
		return this.#name;
	}

	set name(name: string | undefined) {
		this.#name = name;

		this.#broadcast?.close();
		this.#broadcast = undefined;

		this.#run();
	}

	get paused() {
		return this.#paused;
	}

	set paused(paused: boolean) {
		this.#paused = paused;
		this.#reload();
	}

	get volume() {
		return this.#volume;
	}

	set volume(volume: number) {
		this.#volume = volume;
		this.#reload();
	}

	get muted() {
		return this.#muted;
	}

	set muted(muted: boolean) {
		this.#muted = muted;
		this.#reload();
	}

	get latency() {
		return this.#latency;
	}

	set latency(latency: number) {
		this.#latency = latency;

		this.#emitter.latency = this.#latency;
		if (this.#video) {
			this.#video.latency = this.#latency;
		}
	}

	#reload() {
		const volume = this.#muted ? 0 : this.#volume;
		this.#emitter.volume = volume;

		if (this.#audio) {
			this.#audio.enabled = !this.#paused && volume > 0;
		}

		if (this.#video) {
			this.#video.enabled = !this.#paused;
		}
	}

	async #run() {
		this.#stop();

		if (!this.#connection?.established || !this.#name) {
			return;
		}

		this.#broadcast = new BroadcastReload(this.#connection.established, this.#name);

		for (;;) {
			const active = await this.#broadcast.active();
			if (!active) break;

			this.#audio = active.audio;
			this.#video = active.video;

			this.#video.latency = this.#latency;

			this.#emitter.source = this.#audio;
			this.#renderer.source = this.#video;
		}
	}

	#stop() {
		this.#broadcast?.close();
		this.#broadcast = undefined;
	}
}

customElements.define("hang-watch", Element);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": Element;
	}
}

export interface Events {
	connection: keyof Moq.ConnectionStatus;
}

declare global {
	interface HTMLElementEventMap {
		"moq-connection": CustomEvent<keyof Moq.ConnectionStatus>;
	}
}
