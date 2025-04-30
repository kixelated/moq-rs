import * as Moq from "@kixelated/moq";
import * as Media from "../media";
import { Audio } from "./audio";
import { Video } from "./video";

export { Audio, Video };

// A single broadcast, reloading automatically when live/offline.
export class Watch {
	// The connection to the server.
	connection: Moq.Connection;
	#announced: Moq.AnnouncedReader;

	audio: Audio;
	video: Video;

	constructor(connection: Moq.Connection, broadcast: string, audio: Audio, video: Video) {
		this.connection = connection;
		this.#announced = connection.announced(broadcast);
		this.audio = audio;
		this.video = video;

		this.#run().finally(() => this.close());
	}

	async #run() {
		let broadcast: Broadcast | undefined = undefined;

		for (;;) {
			const update = await this.#announced.next();

			// We're donezo.
			if (!update) break;

			// Require full equality.
			if (update.broadcast !== this.#announced.prefix) continue;

			if (update.active) {
				broadcast?.close();
				broadcast = new Broadcast(
					this.connection.clone(),
					this.connection.consume(update.broadcast),
					this.audio,
					this.video,
				);
			} else {
				broadcast?.close();
			}
		}
	}

	close() {
		this.connection.close();
		this.#announced.close();
	}
}

// An established broadcast that reloads on catalog changes.
export class Broadcast {
	connection: Moq.Connection;
	broadcast: Moq.BroadcastReader;
	#catalog: Moq.TrackReader;

	audio: Audio;
	video: Video;

	constructor(connection: Moq.Connection, broadcast: Moq.BroadcastReader, audio: Audio, video: Video) {
		this.connection = connection;
		this.broadcast = broadcast;
		this.#catalog = broadcast.subscribe("catalog.json", 0);
		this.audio = audio;
		this.video = video;

		this.#run().finally(() => this.close());
	}

	async #run() {
		for (;;) {
			const catalog = await Media.Catalog.fetch(this.#catalog);
			if (!catalog) break;

			console.debug("updated catalog", catalog);

			this.video.load(this.broadcast.clone(), catalog.video);
			this.audio.load(this.broadcast.clone(), catalog.audio);
		}
	}

	async close() {
		this.connection.close();
		this.broadcast.close();
		this.video.unload();
		this.audio.unload();
		this.#catalog.close();
	}
}

// A custom element making it easier to insert a Watch into the DOM.
export class Element extends HTMLElement {
	static observedAttributes = ["url", "paused"];

	#reload?: Watch;
	#connection?: Promise<Moq.Connection>;

	audio = new Audio();
	video = new Video();

	constructor() {
		super();

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

		this.addEventListener("click", () => this.audio.init(), { once: true });

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

			this.#reload?.close();
			this.#reload = undefined;

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
			this.#reload = new Watch(connection, broadcast, this.audio, this.video);
			await connection.closed();
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
