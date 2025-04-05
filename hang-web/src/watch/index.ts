import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Container from "../container";
import { Context, Task } from "../util/context";
import { Video } from "./video";

// This class must be created on the main thread due to AudioContext.
export class Watch {
	#url?: string;
	#canvas?: HTMLCanvasElement;

	connecting = new Task(this.connect);
	#loading = new Task(this.#load);

	#catalog?: Catalog.Broadcast;
	#video?: Video;

	get url(): string | undefined {
		return this.#url;
	}

	set url(url: string | undefined) {
		this.#url = url;

		if (url) {
			this.connecting.start(new Context(), url);
		} else {
			this.connecting.abort();
		}
	}

	get catalog() {
		return this.#catalog;
	}

	async connect(context: Context, url: string) {
		// TODO close the pending connection.
		const connection = await context.run(Moq.Connection.connect(url));
		try {
			// Remove the leading slash
			const path = connection.url.pathname.slice(1);
			const announced = await context.run(connection.announced(path));

			for (;;) {
				const announce = await context.run(announced.next());
				if (!announce) {
					return;
				}

				if (!announce.path.endsWith("catalog.json")) {
					continue;
				}

				this.#loading.start(context, connection, announce.path);
			}
		} finally {
			connection.close();
			context.abort();
		}
	}

	async #load(context: Context, connection: Moq.Connection, path: string) {
		const catalog = await context.run(Catalog.Broadcast.fetch(connection, path));

		const video = catalog.video.at(0);
		if (video) {
			this.#video = new Video(video, canvas);
		}

		const audio = catalog.audio.at(0);
		if (audio) {
			//this.#audio.start(context, connection, path, audio);
		} else {
			//this.#audio.abort("no audio");
		}
	}

	async #loadVideo(context: Context, connection: Moq.Connection, path: string, video: Catalog.Video) {
		const track = new Moq.Track(`${path}/${video.track.name}`, video.track.priority);
		const reader = await connection.subscribe(track);
		const container = new Container.Reader(reader);

		try {
			for (;;) {
				const frame = await context.run(container.next());
			}
		} finally {
			container.close();
		}
	}
}
