import * as Catalog from "../hang/catalog";
import type { Connection } from "../lite/connection";
import { Broadcast } from "./broadcast";

export interface PlayerConfig {
	connection: Connection;
	path: string[];
	canvas: HTMLCanvasElement;
}

// This class must be created on the main thread due to AudioContext.
export class Player {
	#config: PlayerConfig;
	#running: Promise<void>;
	#active?: Broadcast;

	constructor(config: PlayerConfig) {
		this.#config = config;
		this.#running = this.#run();
	}

	async #run() {
		const announced = await this.#config.connection.announced(this.#config.path);

		let activeId = -1;

		for (;;) {
			const announce = await announced.next();
			if (!announce) break;

			if (announce.path.length === this.#config.path.length) {
				throw new Error("expected resumable broadcast");
			}

			if (announce.path.length !== this.#config.path.length + 1) {
				// Ignore subtracks
				continue;
			}

			const id = Number.parseInt(announce.path[announce.path.length - 1]);
			if (id <= activeId) {
				console.warn("skipping old broadcast", announce.path);
				continue;
			}

			const catalog = await Catalog.fetch(this.#config.connection, announce.path);

			this.#active?.close();
			this.#active = new Broadcast(this.#config.connection, catalog, this.#config.canvas);
			activeId = id;
		}

		this.#active?.close();
	}

	close() {
		this.#config.connection.close();
		this.#active?.close();
		this.#active = undefined;
	}

	async closed() {
		await Promise.any([this.#running, this.#config.connection.closed()]);
	}

	unmute() {
		this.#active?.unmute();
	}
}
