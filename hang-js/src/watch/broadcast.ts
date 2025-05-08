import * as Moq from "@kixelated/moq";
import * as Media from "../media";
import { AudioTracks } from "./audio";
import { VideoTracks } from "./video";

// A potential broadcast, reloading automatically when live/offline.
export class BroadcastReload {
	// The connection to the server.
	connection: Moq.Connection;
	#announced: Moq.AnnouncedReader;
	#catalog?: Moq.TrackReader;

	#reader: ReadableStreamDefaultReader<Broadcast>;
	#writer: WritableStreamDefaultWriter<Broadcast>;

	constructor(connection: Moq.Connection, broadcast: string) {
		this.connection = connection;
		this.#announced = connection.announced(broadcast);

		const active = new TransformStream<Broadcast, Broadcast>();
		this.#reader = active.readable.getReader();
		this.#writer = active.writable.getWriter();

		this.#run().finally(() => this.close());
	}

	// Returns the next active broadcast.
	async active(): Promise<Broadcast | undefined> {
		const broadcast = await this.#reader.read();
		return broadcast.value;
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
				continue;
			}

			// Create a new broadcast.
			const broadcast = this.connection.consume(update.broadcast);
			this.#writer.write(new Broadcast(broadcast));
		}

		this.#writer.close();
	}

	close() {
		this.#catalog?.close();
		this.#announced.close();
	}
}

// An active broadcast, potentially with audio and video.
export class Broadcast {
	broadcast: Moq.BroadcastReader;
	audio: AudioTracks;
	video: VideoTracks;

	#catalog: Moq.TrackReader;

	constructor(broadcast: Moq.BroadcastReader) {
		this.broadcast = broadcast;
		this.audio = new AudioTracks(broadcast);
		this.video = new VideoTracks(broadcast);

		const catalog = broadcast.subscribe("catalog.json", 0);
		this.#catalog = catalog;

		this.#run().finally(() => this.close());
	}

	async #run() {
		for (;;) {
			const catalog = await Media.Catalog.fetch(this.#catalog);
			if (!catalog) break;

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
