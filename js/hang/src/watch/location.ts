import { cleanup, Signal, signal, Signals } from "@kixelated/signals"
import * as Catalog from "../catalog"
import * as Moq from "@kixelated/moq"

export interface LocationProps {
	enabled?: boolean
}

export class Location {
	enabled: Signal<boolean>

	broadcast: Signal<Moq.BroadcastConsumer | undefined>
	catalog: Signal<Catalog.Root | undefined>

	#current = signal<Catalog.Position | undefined>(undefined)
	readonly current = this.#current.readonly()

	#signals = new Signals()

	constructor(broadcast: Signal<Moq.BroadcastConsumer | undefined>, catalog: Signal<Catalog.Root | undefined>, props?: LocationProps) {
		this.enabled = signal(props?.enabled ?? false)
		this.broadcast = broadcast
		this.catalog = catalog

		this.#signals.effect(this.#run.bind(this))
	}

	#run() {
		if (!this.enabled.get()) return

		const broadcast = this.broadcast.get()
		if (!broadcast) return

		const catalog = this.catalog.get()
		if (!catalog?.location) return

		this.#current.set(catalog.location.initial)
		cleanup(() => this.#current.set(undefined))

		if (!catalog.location.track) return

		const track = broadcast.subscribe(catalog.location.track.name, catalog.location.track.priority)
		cleanup(() => track.close())

		const consumer = new LocationConsumer(track)
		void this.#runConsumer(consumer)

		cleanup(() => {
			consumer.close()
		})
	}

	async #runConsumer(consumer: LocationConsumer) {
		for (; ;) {
			const position = await consumer.next()
			if (!position) break

			this.#current.set(position)
		}

		this.#current.set(undefined)
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

			if (frame.buffer.byteLength !== 8) throw new Error("expected 8 bytes for location")

			const view = new DataView(frame.buffer)
			const x = view.getFloat32(0, true)
			const y = view.getFloat32(4, true)

			return {
				x,
				y,
			}
		} finally {
			group.close()
		}
	}

	close() {
		this.track.close()
	}
}