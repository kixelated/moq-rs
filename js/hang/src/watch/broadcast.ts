import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Connection } from "../connection";
import { Signal, Signals, signal } from "../signals";
import { AudioSource } from "./audio";
import { VideoSource } from "./video";

export type BroadcastProps = {
	// The broadcast path relative to the connection URL.
	// It can be "" if you want to watch the connection's default broadcast.
	path?: string;

	// You can disable reloading if you want to save a round trip when you know the broadcast is already live.
	reload?: boolean;
};

// A broadcast that (optionally) reloads automatically when live/offline.
export class Broadcast {
	connection: Connection;

	path: Signal<string | undefined>;
	status = signal<"offline" | "loading" | "live">("offline");

	audio: AudioSource = new AudioSource();
	video: VideoSource = new VideoSource();

	#broadcast = signal<Moq.BroadcastConsumer | undefined>(undefined);

	#catalog = signal<Catalog.Broadcast | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#active = signal(false);
	readonly active = this.#active.readonly();

	#reload: boolean;
	#signals = new Signals();

	constructor(connection: Connection, props?: BroadcastProps) {
		this.connection = connection;
		this.path = signal(props?.path);
		this.#reload = props?.reload ?? true;

		this.#signals.effect(() => this.#runActive());
		this.#signals.effect(() => this.#runBroadcast());
		this.#signals.effect(() => this.#runCatalog());
		this.#signals.effect(() => this.#runTracks());
	}

	#runActive() {
		if (!this.#reload) {
			this.#active.set(true);

			return () => {
				this.#active.set(false);
			};
		}

		const conn = this.connection.established.get();
		if (!conn) return;

		const path = this.path.get();
		if (path === undefined) return;

		console.log("watching", path);

		const announced = conn.announced(path);
		(async () => {
			for (;;) {
				const update = await announced.next();

				// We're donezo.
				if (!update) break;

				// Require full equality
				if (update.path !== "") {
					console.warn("ignoring suffix", update.path);
					continue;
				}

				this.#active.set(update.active);
			}
		})();

		return () => {
			announced.close();
		};
	}

	#runBroadcast() {
		const conn = this.connection.established.get();
		if (!conn) return;

		const path = this.path.get();
		if (path === undefined) return;

		if (!this.#active.get()) return;

		const broadcast = conn.consume(path);
		this.#broadcast.set(broadcast);

		this.audio.broadcast.set(broadcast);
		this.video.broadcast.set(broadcast);

		return () => {
			broadcast.close();

			this.#broadcast.set(undefined);
			this.audio.broadcast.set(undefined);
			this.video.broadcast.set(undefined);
		};
	}

	#runCatalog() {
		const broadcast = this.#broadcast.get();
		if (!broadcast) return;

		this.status.set("loading");

		const catalog = broadcast.subscribe("catalog.json", 0);

		(async () => {
			try {
				for (;;) {
					const update = await Catalog.Broadcast.fetch(catalog);
					if (!update) break;

					this.#catalog.set(update);
					this.status.set("live");
				}
			} catch (err) {
				console.error("catalog error", err);
			} finally {
				this.#catalog.set(undefined);
				this.status.set("offline");
			}
		})();

		return () => {
			catalog.close();
		};
	}

	#runTracks() {
		const broadcast = this.#broadcast.get();
		if (!broadcast) return;

		const catalog = this.#catalog.get();
		if (!catalog) return;

		this.audio.available.set(catalog.audio);
		this.video.tracks.set(catalog.video);

		return () => {
			this.audio.available.set([]);
			this.video.tracks.set([]);
		};
	}

	close() {
		this.#signals.close();

		this.audio.close();
		this.video.close();
	}
}
