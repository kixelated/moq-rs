import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Container from "../container";
import { Context, Task } from "../util/context";
import { Video } from "./video";

// This class must be created on the main thread due to AudioContext.
export class Watch {
	#url?: string;

	#connecting = new Task(this.#connect);
	#loading = new Task(this.#load);

	#catalog?: Catalog.Broadcast;
	#video?: Video;

	constructor(canvas?: HTMLCanvasElement) {
		this.#video = canvas ? new Video(canvas) : undefined;
	}

	get url(): string | undefined {
		return this.#url;
	}

	set url(url: string | undefined) {
		this.#url = url;

		if (url) {
			this.#connecting.start(url);
		} else {
			this.#connecting.abort();
		}
	}

	get catalog() {
		return this.#catalog;
	}

	async #connect(context: Context, url: string): Promise<Moq.Connection | undefined> {
		await this.#video?.abort();
		await this.#loading.abort();

		// TODO close the pending connection.
		const connection = await context.race(Moq.Connection.connect(url));
		if (!connection) return;

		try {
			// Remove the leading slash
			const path = connection.url.pathname.slice(1);
			const announced = await context.race(connection.announced(path));
			if (!announced) return;

			for (;;) {
				const announce = await context.race(announced.next());
				if (!announce) return;

				if (!announce.path.endsWith("catalog.json")) {
					continue;
				}

				// TODO Only cancel the previous task after a successful (async) load.
				this.#loading.start(connection, announce.path);
			}
		} finally {
			connection.close();
			context.abort();
		}
	}

	async #load(context: Context, connection: Moq.Connection, path: string) {
		const catalog = await context.race(Catalog.Broadcast.fetch(connection, path));
		if (!catalog) return;

		if (this.#video) {
			await this.#video.load(connection, path, catalog.video);
		}
	}
}
