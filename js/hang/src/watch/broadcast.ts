import * as Moq from "@kixelated/moq"
import { Signal, Signals, signal } from "@kixelated/signals"
import * as Catalog from "../catalog"
import { Connection } from "../connection"
import { Audio, AudioProps } from "./audio"
import { Video, VideoProps } from "./video"
import { Location, LocationProps } from "./location"

export interface BroadcastProps {
	// The broadcast path relative to the connection URL.
	// Defaults to ""
	path?: string

	// You can disable reloading if you want to save a round trip when you know the broadcast is already live.
	reload?: boolean

	video?: VideoProps
	audio?: AudioProps
	location?: LocationProps
}

// A broadcast that (optionally) reloads automatically when live/offline.
export class Broadcast {
	connection: Connection

	path: Signal<string>
	status = signal<"offline" | "loading" | "live">("offline");

	audio: Audio
	video: Video
	location: Location

	#broadcast = signal<Moq.BroadcastConsumer | undefined>(undefined);

	#catalog = signal<Catalog.Root | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#active = signal(false);
	readonly active = this.#active.readonly();

	#reload: boolean
	#signals = new Signals();

	constructor(connection: Connection, props?: BroadcastProps) {
		this.connection = connection
		this.path = signal(props?.path ?? "")
		this.audio = new Audio(props?.audio)
		this.video = new Video(props?.video)
		this.location = new Location(props?.location)
		this.#reload = props?.reload ?? true

		this.#signals.effect(() => this.#runActive())
		this.#signals.effect(() => this.#runBroadcast())
		this.#signals.effect(() => this.#runCatalog())
		this.#signals.effect(() => this.#runTracks())
	}

	#runActive() {
		if (!this.#reload) {
			this.#active.set(true)

			return () => {
				this.#active.set(false)
			}
		}

		const conn = this.connection.established.get()
		if (!conn) return

		const path = this.path.get()

		const announced = conn.announced(path);
		(async () => {
			for (; ;) {
				const update = await announced.next()

				// We're donezo.
				if (!update) break

				// Require full equality
				if (update.path !== "") {
					console.warn("ignoring suffix", update.path)
					continue
				}

				this.#active.set(update.active)
			}
		})()

		return () => {
			announced.close()
		}
	}

	#runBroadcast() {
		const conn = this.connection.established.get()
		if (!conn) return

		const path = this.path.get()
		if (!this.#active.get()) return

		const broadcast = conn.consume(path)
		this.#broadcast.set(broadcast)

		this.audio.broadcast.set(broadcast)
		this.video.broadcast.set(broadcast)
		this.location.broadcast.set(broadcast)

		return () => {
			broadcast.close()

			this.#broadcast.set(undefined)
			this.audio.broadcast.set(undefined)
			this.video.broadcast.set(undefined)
			this.location.broadcast.set(undefined)
		}
	}

	#runCatalog() {
		const broadcast = this.#broadcast.get()
		if (!broadcast) return

		this.status.set("loading")

		const catalog = broadcast.subscribe("catalog.json", 0);

		(async () => {
			try {
				for (; ;) {
					const update = await Catalog.Root.fetch(catalog)
					if (!update) break

					this.#catalog.set(update)
					this.status.set("live")
				}
			} catch (err) {
				console.error("catalog error", err)
			} finally {
				this.#catalog.set(undefined)
				this.status.set("offline")
			}
		})()

		return () => {
			catalog.close()
		}
	}

	#runTracks() {
		const broadcast = this.#broadcast.get()
		if (!broadcast) return

		const catalog = this.#catalog.get()
		if (!catalog) return

		this.audio.catalog.set(catalog.audio)
		this.video.catalog.set(catalog.video)
		this.location.catalog.set(catalog.location)

		return () => {
			this.audio.catalog.set([])
			this.video.catalog.set([])
		}
	}

	close() {
		this.#signals.close()

		this.audio.close()
		this.video.close()
	}
}
