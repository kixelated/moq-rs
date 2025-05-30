import { Signal, signal, Signals } from "@kixelated/signals"
import * as Catalog from "../catalog"
import * as Moq from "@kixelated/moq"

export type LocationProps = {
	// If true, then we'll publish our position to the broadcast.
	enabled?: boolean

	// Our initial position.
	position?: Catalog.Position

	// If provided, this broadcaster allows other peers to request a position update via this (random) handle.
	handle?: number
}

export class Location {
	broadcast: Moq.BroadcastProducer

	enabled: Signal<boolean>

	position: Signal<Catalog.Position | undefined>

	// If provided, this broadcaster allows other peers to request a position update via this (random) handle.
	handle: Signal<number | undefined>

	#track = new Moq.TrackProducer("location", 0);
	#producer = new LocationProducer(this.#track);

	#catalog = signal<Catalog.Location | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#signals = new Signals();

	constructor(broadcast: Moq.BroadcastProducer, props?: LocationProps) {
		this.broadcast = broadcast

		this.enabled = signal(props?.enabled ?? false)
		this.position = signal(props?.position ?? undefined)
		this.handle = signal(props?.handle ?? undefined)

		broadcast.insertTrack(this.#track.consume())
		this.#signals.cleanup(() => broadcast.removeTrack(this.#track.name))

		this.#signals.effect(() => this.#runCatalog())
		this.#signals.effect(() => this.#runProducer())
	}

	// Request that a peer update their position via their handle.
	peer(handle: number): LocationProducer {
		const track = new Moq.TrackProducer(`location/${handle}`, 0)
		const producer = new LocationProducer(track)
		this.broadcast.insertTrack(track.consume())

		this.#catalog.set((prev) => {
			return {
				...(prev?.peers ?? {}),
				[handle]: {
					track: {
						name: track.name,
						priority: track.priority,
					},
				},
			}
		})

		track.closed().then(() => {
			this.broadcast.removeTrack(track.name)

			// TODO also remove from catalog
			this.#catalog.set((prev) => {
				const { [handle]: _, ...rest } = prev?.peers ?? {}
				return {
					...prev,
					peers: rest,
				}
			})
		})

		return producer
	}

	#runProducer() {
		const position = this.position.get()
		if (!position) return

		this.#producer.update(position)
	}

	#runCatalog() {
		const enabled = this.enabled.get()
		if (!enabled) return

		const catalog = {
			initial: this.position.peek(), // Doesn't trigger a re-render
			updates: { name: this.#track.name, priority: this.#track.priority },
			handle: this.handle.get(),
		}

		this.#catalog.set(catalog)
	}

	close() {
		this.#producer.close()
		this.#signals.close()
	}
}

export class LocationProducer {
	track: Moq.TrackProducer

	constructor(track: Moq.TrackProducer) {
		this.track = track
	}

	update(position: Catalog.Position) {
		const group = this.track.appendGroup()

		// Everything is encoded as a f32 for simplicity.
		// We can definitely same a few bytes if needed later.
		const buffer = new ArrayBuffer(12)
		const view = new DataView(buffer)
		view.setFloat32(0, position.x ?? 0)
		view.setFloat32(4, position.y ?? 0)
		view.setFloat32(8, position.zoom ?? 1)

		group.writeFrame(new Uint8Array(buffer))
		group.close()
	}

	close() {
		this.track.close()
	}
}
