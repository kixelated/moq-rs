import * as Moq from "@kixelated/moq";
import { BroadcastReload } from "./broadcast";
import { VideoRenderer } from "./video";
import { AudioEmitter } from "./audio";

// A custom element that renders to a canvas.
export class Element extends HTMLElement {
	static observedAttributes = ["url", "paused"];

	#broadcast?: BroadcastReload;
	#connection?: Promise<Moq.Connection>;

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
				this.audio.paused = this.video.paused;
			},
			{ once: true },
		);

		this.attachShadow({ mode: "open" }).append(style, slot);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			void this.#connect(newValue);
		} else if (name === "paused") {
			this.video.paused = newValue !== undefined;
			this.audio.paused = newValue !== undefined;
		}
	}

	async #connect(attr?: string) {
		const url = attr ? new URL(attr) : undefined;
		if (!url) {
			const existing = await this.#connection;
			existing?.close();

			this.#connection = undefined;

			this.#broadcast?.close();
			this.#broadcast = undefined;

			return;
		}

		const broadcast = url.pathname.slice(1);

		// Connect to the URL without the path
		const base = new URL(url);
		base.pathname = "";

		this.#connection = Moq.Connection.connect(base);

		try {
			this.#dispatchEvent("connection", "connecting");
			const connection = await this.#connection;
			this.#dispatchEvent("connection", "connected");
			this.#broadcast = new BroadcastReload(connection, broadcast);

			for (;;) {
				const active = await this.#broadcast.active();
				if (active) {
					this.audio.broadcast = active.audio;
					this.video.broadcast = active.video;
				}
			}
		} finally {
			this.#dispatchEvent("connection", "disconnected");
		}
	}

	#dispatchEvent<K extends keyof Events>(type: K, detail: Events[K]) {
		this.dispatchEvent(new CustomEvent(`hang-watch-${type}`, { detail }));
	}
}

customElements.define("hang-watch", Element);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": Element;
	}
}

export interface Events {
	connection: ConnectionStatus;
}

export type ConnectionStatus = "connecting" | "connected" | "disconnected";

declare global {
	interface HTMLElementEventMap {
		"hang-watch-connection": CustomEvent<ConnectionStatus>;
	}
}
