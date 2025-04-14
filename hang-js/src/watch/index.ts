import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Context, Task } from "../util/context";
import { Video } from "./video";

export class Watch {
	#url?: URL;

	#connection = new Task(this.#connect.bind(this));

	#catalog?: Catalog.Broadcast;
	#video = new Video();

	#latency = 0;
	#paused = false;

	get url(): URL | undefined {
		return this.#url;
	}

	set url(url: URL | undefined) {
		this.#url = url;

		if (url) {
			this.#connection.start(url);
		} else {
			this.#connection.abort();
		}
	}

	get catalog(): Catalog.Broadcast | undefined {
		return this.#catalog;
	}

	get latency(): number {
		return this.#latency;
	}

	set latency(latency: number) {
		this.#latency = latency;
		this.#video.latency = latency;
	}

	get paused(): boolean {
		return this.#paused;
	}

	set paused(paused: boolean) {
		this.#paused = paused;
	}

	get canvas(): HTMLCanvasElement | undefined {
		return this.#video.canvas;
	}

	set canvas(canvas: HTMLCanvasElement | undefined) {
		this.#video.canvas = canvas;
	}

	async #connect(context: Context, url: URL) {
		const path = url.pathname.slice(1);

		// Connect to the URL without the path
		const base = new URL(url);
		base.pathname = "";

		const connection = await context.race(Moq.Connection.connect(base));

		try {
			const announced = await context.race(connection.announced(path));

			for (;;) {
				const announce = await context.race(announced.next());
				if (!announce) break;
				if (!announce.path.endsWith("catalog.json")) continue;

				Catalog.Broadcast.fetch(connection, path).then((catalog) => {
					this.#video.load(connection, path, catalog.video);
				});
			}
		} finally {
			connection.close();
		}
	}
}

// A custom element making it easier to insert a Watch into the DOM.
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url", "latency", "paused"];

	#watch: Watch = new Watch();

	constructor() {
		super();

		const canvas = document.createElement("canvas");

		const slot = document.createElement("slot");
		for (const el of slot.assignedElements({ flatten: true })) {
			this.#watch.canvas = el as HTMLCanvasElement;
		}

		slot.appendChild(canvas);
		this.attachShadow({ mode: "open" }).appendChild(slot);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.#watch.url = newValue ? new URL(newValue) : undefined;
		} else if (name === "latency") {
			this.#watch.latency = newValue ? Number.parseInt(newValue) : 0;
		} else if (name === "paused") {
			this.#watch.paused = newValue !== undefined;
		}
	}
}

customElements.define("hang-watch", WatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": WatchElement;
	}
}

export default WatchElement;
