import * as Moq from "@kixelated/moq";
import { Signal, Signals, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";
import { Connection } from "../connection";
import { Audio, AudioProps } from "./audio";
import { Location, LocationProps } from "./location";
import { Video, VideoProps } from "./video";

export interface BroadcastProps {
	// Whether to start downloading the broadcast.
	// Defaults to false so you can make sure everything is ready before starting.
	enabled?: boolean;

	// The broadcast path relative to the connection URL.
	// Defaults to ""
	path?: string;

	// You can disable reloading if you want to save a round trip when you know the broadcast is already live.
	reload?: boolean;

	video?: VideoProps;
	audio?: AudioProps;
	location?: LocationProps;
}

// A broadcast that (optionally) reloads automatically when live/offline.
export class Broadcast {
	connection: Connection;

	enabled: Signal<boolean>;
	path: Signal<string>;
	status = signal<"offline" | "loading" | "live">("offline");

	audio: Audio;
	video: Video;
	location: Location;

	#broadcast = signal<Moq.BroadcastConsumer | undefined>(undefined);

	#catalog = signal<Catalog.Root | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	// This signal is true when the broadcast has been announced, unless reloading is disabled.
	#active = signal(false);
	readonly active = this.#active.readonly();

	#reload: boolean;
	#signals = new Signals();

	constructor(connection: Connection, props?: BroadcastProps) {
		this.connection = connection;
		this.path = signal(props?.path ?? "");
		this.enabled = signal(props?.enabled ?? false);
		this.audio = new Audio(this.#broadcast, this.#catalog, props?.audio);
		this.video = new Video(this.#broadcast, this.#catalog, props?.video);
		this.location = new Location(this.#broadcast, this.#catalog, props?.location);
		this.#reload = props?.reload ?? true;

		this.#signals.effect(() => this.#runActive());
		this.#signals.effect(() => this.#runBroadcast());
		this.#signals.effect(() => this.#runCatalog());
	}

	#runActive() {
		if (!this.enabled.get()) return;

		if (!this.#reload) {
			this.#active.set(true);

			return () => {
				this.#active.set(false);
			};
		}

		const conn = this.connection.established.get();
		if (!conn) return;

		const path = this.path.get();

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

		if (!this.enabled.get()) return;

		const path = this.path.get();
		if (!this.#active.get()) return;

		const broadcast = conn.consume(path);
		this.#broadcast.set(broadcast);

		return () => {
			broadcast.close();
			this.#broadcast.set(undefined);
		};
	}

	#runCatalog() {
		if (!this.enabled.get()) return;

		const broadcast = this.#broadcast.get();
		if (!broadcast) return;

		this.status.set("loading");

		const catalog = broadcast.subscribe("catalog.json", 0);

		(async () => {
			try {
				for (;;) {
					const update = await Catalog.fetch(catalog);
					if (!update) break;

					this.#catalog.set(update);
					this.status.set("live");
				}
			} finally {
				this.#catalog.set(undefined);
				this.status.set("offline");
			}
		})();

		return () => {
			catalog.close();
		};
	}

	close() {
		this.#signals.close();

		this.audio.close();
		this.video.close();
		this.location.close();
	}
}
