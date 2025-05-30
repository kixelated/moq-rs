import { cleanup, Derived, Signal, signal, Signals } from "@kixelated/signals"
import * as Catalog from "../catalog"
import * as Moq from "@kixelated/moq"

export interface LocationProps {
	enabled?: boolean
}

export class Location {
	enabled: Signal<boolean>

	broadcast: Signal<Moq.BroadcastConsumer | undefined>
	catalog: Derived<Catalog.Location | undefined>
	handle: Derived<number | undefined>

	#current = signal<Catalog.Position | undefined>(undefined)
	readonly current = this.#current.readonly()

	#signals = new Signals()

	constructor(broadcast: Signal<Moq.BroadcastConsumer | undefined>, catalog: Signal<Catalog.Root | undefined>, props?: LocationProps) {
		this.enabled = signal(props?.enabled ?? false)
		this.broadcast = broadcast
		this.catalog = this.#signals.derived(() => {
			return this.enabled.get() ? catalog.get()?.location : undefined
		})
		this.handle = this.#signals.derived(() => this.catalog.get()?.handle)

		this.#signals.effect(this.#run.bind(this))
	}

	#run() {
		const broadcast = this.broadcast.get()
		if (!broadcast) return

		const catalog = this.catalog.get()
		if (!catalog) return

		this.#current.set(catalog.initial)
		cleanup(() => this.#current.set(undefined))

		const updates = catalog.updates
		if (!updates) return

		const track = broadcast.subscribe(updates.name, updates.priority)
		cleanup(() => track.close())

		const consumer = new LocationConsumer(track)
		void this.#runConsumer(consumer, this.#current)

		cleanup(() => consumer.close())
	}

	// Request a reactive signal for a specific handle on demand.
	// This is useful when publishing, as you only want to subscribe to the feedback you need.
	peer(handle: number): Derived<Catalog.Position | undefined> {
		const location = signal<Catalog.Position | undefined>(undefined)
		this.#signals.effect(() => this.#runPeer(handle, location))

		return location.readonly()
	}

	#runPeer(handle: number, location: Signal<Catalog.Position | undefined>) {
		cleanup(() => location.set(undefined))

		const broadcast = this.broadcast.get()
		if (!broadcast) return

		const catalog = this.catalog.get()
		if (!catalog) return

		const track = catalog.peers?.[handle]
		if (!track) return

		const sub = broadcast.subscribe(track.name, track.priority)
		cleanup(() => sub.close())

		const consumer = new LocationConsumer(sub)
		void this.#runConsumer(consumer, location)
	}

	async #runConsumer(consumer: LocationConsumer, location: Signal<Catalog.Position | undefined>) {
		for (; ;) {
			const position = await consumer.next()
			if (!position) break

			location.set(position)
		}

		location.set(undefined)
	}

	close() {
		this.#signals.close()
	}
}

export class LocationConsumer {
	track: Moq.TrackConsumer

	constructor(track: Moq.TrackConsumer) {
		this.track = track
	}

	async next(): Promise<Catalog.Position | undefined> {
		const group = await this.track.nextGroup()
		if (!group) return undefined

		try {
			const frame = await group.readFrame()
			if (!frame) return undefined

			let x: number | undefined
			let y: number | undefined
			let zoom: number | undefined

			const view = new DataView(frame.buffer)
			if (view.byteLength >= 4) {
				x = view.getFloat32(0)
			}
			if (view.byteLength >= 8) {
				y = view.getFloat32(4)
			}
			if (view.byteLength >= 12) {
				zoom = view.getFloat32(8)
			}

			return {
				x,
				y,
				zoom,
			}
		} finally {
			group.close()
		}
	}

	close() {
		this.track.close()
	}
}
