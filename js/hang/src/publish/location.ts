import { Signal, signal, Signals } from "@kixelated/signals"
import * as Catalog from "../catalog"
import * as Moq from "@kixelated/moq"

export type LocationProps = {
	// If true, then we'll publish our position to the broadcast.
	enabled?: boolean

	// Our initial position.
	position?: Catalog.Position
}

export class Location {
	enabled: Signal<boolean>
	current: Signal<Catalog.Position | undefined>

	#track = new Moq.TrackProducer("location", 0);
	#producer = new LocationProducer(this.#track);

	#catalog = signal<Catalog.Location | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#signals = new Signals();

	constructor(broadcast: Moq.BroadcastProducer, props?: LocationProps) {
		this.enabled = signal(props?.enabled ?? false)
		this.current = signal(props?.position ?? undefined)

		broadcast.insertTrack(this.#track.consume())
		this.#signals.cleanup(() => broadcast.removeTrack(this.#track.name))

		this.#signals.effect(() => this.#runCatalog())
		this.#signals.effect(() => this.#runProducer())
	}

	#runProducer() {
		const position = this.current.get()
		if (!position) return

		this.#producer.append(position)
	}

	#runCatalog() {
		const enabled = this.enabled.get()
		if (!enabled) return

		const catalog = {
			initial: this.current.peek(), // Doesn't trigger a re-render
			track: { name: this.#track.name, priority: this.#track.priority },
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

	append({ x, y }: { x: number, y: number }) {
		const group = this.track.appendGroup()

		// We encode our x as a f32, and our y as a f32.
		const buffer = new ArrayBuffer(8)
		const view = new DataView(buffer)
		view.setFloat32(0, x, true)
		view.setFloat32(4, y, true)

		group.writeFrame(new Uint8Array(buffer))
		group.close()
	}

	close() {
		this.track.close()
	}
}