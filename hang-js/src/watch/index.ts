import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Audio } from "./audio";
import { Video } from "./video";

export interface WatchEvents {
	status: WatchStatus;
}

export type WatchStatus = "connecting" | "connected" | "disconnected" | "live" | "offline" | "error";

export class Watch {
	#url?: URL;

	// The connection to the server.
	#connection?: Promise<Moq.Connection>;

	// Handlers for audio and video components.
	audio = new Audio();
	video = new Video();

	// Fires events.
	#events = new EventTarget();

	get url(): URL | undefined {
		return this.#url;
	}

	set url(url: URL | undefined) {
		this.#url = url;

		// Close the old connection.
		this.#connection?.then((connection) => connection.close()).catch(() => {});

		if (url) {
			this.#dispatchEvent("status", "connecting");
			this.#connection = this.#connect(url);
		} else {
			this.#dispatchEvent("status", "disconnected");
			this.#connection = undefined;
		}
	}

	async #connect(url: URL) {
		const path = url.pathname.slice(1);

		// Connect to the URL without the path
		const base = new URL(url);
		base.pathname = "";

		const connection = await Moq.Connection.connect(base);
		this.#dispatchEvent("status", "connected");

		//this.#announcement?.close();
		const announcement = connection.announced(path);

		this.#runAnnouncement(connection, announcement);

		// Return the connection so we can close it if needed.
		return connection;
	}

	async #runAnnouncement(connection: Moq.Connection, announcement: Moq.AnnouncedReader) {
		let broadcast: Moq.BroadcastReader | undefined;
		let catalog: Moq.TrackReader | undefined;

		this.#dispatchEvent("status", "offline");

		try {
			for (;;) {
				const update = await announcement.next();

				// We're donezo.
				if (!update) break;

				// Require full equality.
				if (update.broadcast !== announcement.prefix) continue;

				if (update.active) {
					console.debug("broadcast is live", update);
					this.#dispatchEvent("status", "live");

					broadcast = connection.consume(update.broadcast);
					catalog = broadcast.subscribe("catalog.json", 0);

					this.#runCatalog(broadcast, catalog);
				} else {
					this.#dispatchEvent("status", "offline");

					broadcast?.close();
					catalog?.close();
				}
			}
		} finally {
			announcement.close();
			broadcast?.close();
			catalog?.close();
		}
	}

	async #runCatalog(broadcast: Moq.BroadcastReader, track: Moq.TrackReader) {
		try {
			for (;;) {
				const catalog = await Catalog.Broadcast.fetch(track);
				if (!catalog) break;

				console.debug("updated catalog", catalog);

				this.video.load(broadcast, catalog.video);
				this.audio.load(broadcast, catalog.audio);
			}
		} finally {
			track.close();
			broadcast.close();
			this.video.close();
			this.audio.close();

			this.#dispatchEvent("status", "offline");
		}
	}

	close() {
		this.#connection?.then((connection) => connection.close()).catch(() => {});
		this.#connection = undefined;

		this.audio.close();
		this.video.close();

		this.#dispatchEvent("status", "disconnected");
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
}

// A custom element making it easier to insert a Watch into the DOM.
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url", "paused", "maxLatency"];

	// Expose the library so we don't have to duplicate everything.
	readonly lib: Watch = new Watch();

	constructor() {
		super();

		// Proxy events from the library to the element.
		this.lib.addEventListener("status", (event) => {
			this.dispatchEvent(new CustomEvent("hang-watch-status", { detail: event.detail }));
		});

		const canvas = document.createElement("canvas");
		canvas.style.width = "100%";
		canvas.style.height = "auto";

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLCanvasElement) {
					this.lib.video.canvas = el;
					return;
				}
			}

			this.lib.video.canvas = undefined;
		});

		slot.appendChild(canvas);
		this.lib.video.canvas = canvas;

		const style = document.createElement("style");
		style.textContent = `
			:host {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		`;

		this.addEventListener("click", () => this.lib.audio.init(), { once: true });

		this.attachShadow({ mode: "open" }).append(style, slot);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.url = newValue ? new URL(newValue) : undefined;
		} else if (name === "paused") {
			this.lib.video.paused = newValue !== undefined;
			this.lib.audio.paused = newValue !== undefined;
		} else if (name === "maxLatency") {
			// Latency in seconds.
			this.lib.video.maxLatency = Number.parseFloat(newValue ?? "0");
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
		"hang-watch-status": CustomEvent<WatchStatus>;
	}
}
