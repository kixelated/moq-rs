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

	readonly track = new Moq.TrackProducer("location", 0);

	#catalog = signal<Catalog.Location | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	position: Signal<Catalog.Position>

	#signals = new Signals();

	constructor(props?: LocationProps) {
		this.enabled = signal(props?.enabled ?? false)
		this.position = signal(props?.position ?? { x: 0, y: 0 })

		this.#signals.effect(() => this.#runCatalog())
		this.#signals.effect(() => this.#run())
	}

	#run() {
		const position = this.position.get()
		const group = this.track.appendGroup()

		// We encode our x as a f32, and our y as a f32.
		const buffer = new ArrayBuffer(8)
		const view = new DataView(buffer)
		view.setFloat32(0, position.x, true)
		view.setFloat32(4, position.y, true)

		group.writeFrame(new Uint8Array(buffer))
		group.close()
	}

	#runCatalog() {
		const enabled = this.enabled.get()
		if (!enabled) return

		const catalog = {
			initial: this.position.peek(),
			track: { name: this.track.name, priority: this.track.priority },
		}

		this.#catalog.set(catalog)
	}

	close() {
		this.#signals.close()
	}
}