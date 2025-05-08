import * as Moq from "@kixelated/moq";

import { AudioEmitter } from "./audio";
import { BroadcastReload } from "./broadcast";
import { VideoRenderer } from "./video";

// A custom element that renders to a canvas.
export class Element extends HTMLElement {
	static observedAttributes = ["url", "name", "paused"];

	#url?: URL;
	#name?: string;
	#paused = false;

	#connection?: Moq.ConnectionReload;

	#broadcast?: BroadcastReload;
	audio = new AudioEmitter();
	video = new VideoRenderer();

	constructor() {
		super();

		// Set the maximum latency for the audio and video so they're "synchronized".
		// TODO this is pretty high because the BBB audio stutters; fix that.
		this.audio.latency = 100;
		this.video.latency = 100;

		const canvas = document.createElement("canvas");
		canvas.style.width = "100%";
		canvas.style.height = "auto";

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLCanvasElement) {
					this.video.canvas = el;
					return;
				}
			}

			this.video.canvas = undefined;
		});

		slot.appendChild(canvas);
		this.video.canvas = canvas;

		const style = document.createElement("style");
		style.textContent = `
			:host {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		`;

		// We can only start audio playback once the user has clicked the element.
		this.addEventListener(
			"click",
			() => {
				// When the user clicks the element, we start the audio if the video is playing.
				this.audio.muted = this.video.paused;
			},
			{ once: true },
		);

		this.attachShadow({ mode: "open" }).append(style, slot);
	}

	attributeChangedCallback(
		name: string,
		oldValue: string | undefined,
		newValue: string | undefined,
	) {
		if (oldValue === newValue) {
			return;
		}

		if (name === "url") {
			this.url = newValue ? new URL(newValue) : undefined;
		} else if (name === "name") {
			this.name = newValue;
		} else if (name === "paused") {
			this.paused = newValue !== undefined;
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
			this.dispatchEvent(
				new CustomEvent("moq-connection", { detail: "connecting" }),
			);
		});

		this.#connection.on("connected", (_connection) => {
			this.dispatchEvent(
				new CustomEvent("moq-connection", { detail: "connected" }),
			);
			this.#run();
		});

		this.#connection.on("disconnected", () => {
			this.dispatchEvent(
				new CustomEvent("moq-connection", { detail: "disconnected" }),
			);
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

		this.video.paused = paused;
		this.audio.muted = paused;
	}

	async #run() {
		this.#stop();

		if (!this.#connection?.established || !this.#name) {
			return;
		}

		this.#broadcast = new BroadcastReload(
			this.#connection.established,
			this.#name,
		);

		for (;;) {
			const active = await this.#broadcast.active();
			if (active) {
				this.audio.broadcast = active.audio;
				this.video.broadcast = active.video;
			}
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
