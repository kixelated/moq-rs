import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Video } from "./video";

export interface WatchEvents {
	status: WatchStatus;
}

export type WatchStatus = "connecting" | "connected" | "disconnected" | "live" | "offline" | "error";

export class Watch {
	#url?: URL;

	#connection?: Promise<Moq.Connection>;
	#catalog?: Moq.TrackReader;
	#video = new Video();

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

	get paused(): boolean {
		return this.#video.paused;
	}

	set paused(paused: boolean) {
		this.#video.paused = paused;
	}

	get canvas(): HTMLCanvasElement | undefined {
		return this.#video.canvas;
	}

	set canvas(canvas: HTMLCanvasElement | undefined) {
		this.#video.canvas = canvas;
	}

	async #connect(url: URL) {
		const broadcast = url.pathname.slice(1);

		// Connect to the URL without the path
		const base = new URL(url);
		base.pathname = "";

		const connection = await Moq.Connection.connect(base);
		this.#dispatchEvent("status", "connected");

		this.#catalog?.close();
		this.#catalog = connection.subscribe(broadcast, "catalog.json");

		this.#runCatalog(connection, broadcast, this.#catalog);

		// Return the connection so we can close it if needed.
		return connection;
	}

	async #runCatalog(connection: Moq.Connection, broadcast: string, track: Moq.TrackReader) {
		try {
			for (;;) {
				const catalog = await Catalog.Broadcast.fetch(track);
				if (!catalog) break;

				this.#dispatchEvent("status", "live");
				this.#video.catalog(connection, broadcast, catalog.video);
			}
		} finally {
			this.#dispatchEvent("status", "offline");
			this.#video.close();
		}
	}

	close() {
		this.#connection?.then((connection) => connection.close()).catch(() => {});
		this.#connection = undefined;
		this.#catalog?.close();
		this.#catalog = undefined;

		this.#video.close();

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
	static observedAttributes = ["url", "paused"];

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
					this.lib.canvas = el;
					return;
				}
			}

			this.lib.canvas = undefined;
		});

		slot.appendChild(canvas);
		this.lib.canvas = canvas;

		this.attachShadow({ mode: "open" }).appendChild(slot);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.url = newValue ? new URL(newValue) : undefined;
		} else if (name === "paused") {
			this.lib.paused = newValue !== undefined;
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

export default WatchElement;
