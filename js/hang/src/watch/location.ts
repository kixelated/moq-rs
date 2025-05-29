import { cleanup, Signal, signal, Signals } from "@kixelated/signals"
import * as Catalog from "../catalog"
import * as Moq from "@kixelated/moq"

export interface LocationProps {
	enabled?: boolean
	broadcast?: Moq.BroadcastConsumer
	catalog?: Catalog.Location
}

export class Location {
	enabled: Signal<boolean>

	broadcast: Signal<Moq.BroadcastConsumer | undefined>
	catalog: Signal<Catalog.Location | undefined>

	#current = signal<Catalog.Position | undefined>(undefined)
	readonly current = this.#current.readonly()

	#signals = new Signals()

	constructor(props?: LocationProps) {
		this.enabled = signal(props?.enabled ?? false)
		this.catalog = signal(props?.catalog)
		this.broadcast = signal(props?.broadcast)

		this.#signals.effect(this.#run.bind(this))
	}

	#run() {
		if (!this.enabled.get()) return

		const broadcast = this.broadcast.get()
		if (!broadcast) return

		const catalog = this.catalog.get()
		if (!catalog) return

		this.#current.set(catalog.initial)
		cleanup(() => this.#current.set(undefined))

		if (!catalog.track) return

		const track = broadcast.subscribe(catalog.track.name, catalog.track.priority)
		cleanup(() => track.close())

		void this.#runTrack(track)
	}

	async #runTrack(track: Moq.TrackConsumer) {
		for (; ;) {
			const group = await track.nextGroup()
			if (!group) return

			try {
				const frame = await group.readFrame()
				if (!frame) return

				if (frame.buffer.byteLength !== 8) throw new Error("expected 8 bytes for location")

				// The next 8 bytes are the location encoded as two float32.
				const view = new DataView(frame.buffer)
				const x = view.getFloat32(0, true)
				const y = view.getFloat32(4, true)

				this.#current.set({
					x,
					y,
				})
			} finally {
				group.close()
			}
		}
	}

	close() {
		this.#signals.close()
	}
}