import * as Moq from "@kixelated/moq";
import { Watch } from "@kixelated/moq/util";
import * as Media from "../media";
import { AudioSource } from "./audio";
import { VideoSource } from "./video";

// A potential broadcast, reloading automatically when live/offline.
export class BroadcastReload {
	// The connection to the server.
	connection: Moq.Connection;
	#announced: Moq.AnnouncedReader;
	#catalog?: Moq.TrackReader;

	#watch = new Watch<Broadcast | null>(null);

	constructor(connection: Moq.Connection, broadcast: string) {
		this.connection = connection;
		this.#announced = connection.announced(broadcast);

		this.#run().finally(() => this.close());
	}

	// Returns the next active broadcast.
	async active(): Promise<Broadcast | undefined> {
		const next = await this.#watch.consumer.next((v) => !!v);
		return next ?? undefined;
	}

	async #run() {
		for (;;) {
			const update = await this.#announced.next();

			// We're donezo.
			if (!update) break;

			// Require full equality.
			if (update.broadcast !== this.#announced.prefix) continue;

			// Close the previous catalog.
			this.#catalog?.close();

			if (!update.active) {
				this.#watch.producer.update((old) => {
					if (old) old.close();
					return null;
				});

				continue;
			}

			// Create a new broadcast.
			const broadcast = this.connection.consume(update.broadcast);
			this.#watch.producer.update((old) => {
				if (old) old.close();
				return new Broadcast(broadcast);
			});
		}

		this.#watch.producer.close();
	}

	close() {
		this.#catalog?.close();
		this.#announced.close();
	}
}

// An active broadcast, potentially with audio and video.
export class Broadcast {
	broadcast: Moq.BroadcastReader;
	audio: AudioSource;
	video: VideoSource;

	#catalog: Moq.TrackReader;

	constructor(broadcast: Moq.BroadcastReader) {
		this.broadcast = broadcast;
		this.audio = new AudioSource(broadcast);
		this.video = new VideoSource(broadcast);

		const catalog = broadcast.subscribe("catalog.json", 0);
		this.#catalog = catalog;

		this.#run().finally(() => this.close());
	}

	async #run() {
		for (;;) {
			const catalog = await Media.Catalog.fetch(this.#catalog);
			if (!catalog) break;

			console.log("received catalog", this.broadcast.path, catalog);

			this.audio.tracks = catalog.audio;
			this.video.tracks = catalog.video;
		}
	}

	close() {
		this.broadcast.close();
		this.audio.close();
		this.video.close();
		this.#catalog.close();
	}
}
