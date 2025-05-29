import { cleanup, Derived, Signal, signal, Signals } from "@kixelated/signals"

import * as Moq from "@kixelated/moq"
import * as Catalog from "../catalog"
import { isEqual } from "lodash"
import { LocationConsumer } from "./location"

export interface FeedbackProps {
	enabled?: boolean
}

// This is feedback that we are receiving from the broadcaster.
export class Feedback {
	enabled: Signal<boolean>
	broadcast: Signal<Moq.BroadcastConsumer | undefined>
	catalog: Signal<Catalog.Root | undefined>

	capabilities: Derived<Catalog.Capabilities | undefined>

	#signals = new Signals()

	constructor(broadcast: Signal<Moq.BroadcastConsumer | undefined>, catalog: Signal<Catalog.Root | undefined>, props?: FeedbackProps) {
		this.enabled = signal(props?.enabled ?? false)
		this.broadcast = broadcast
		this.catalog = catalog

		this.capabilities = this.#signals.derived(() => this.catalog.get()?.feedback?.capabilities, {
			equals: (a, b) => isEqual(a, b),
		})
	}

	// Request a reactive signal for a specific handle on demand.
	// This is useful when publishing, as you only want to subscribe to the feedback you need.
	location(path: string): Derived<Catalog.Position | undefined> {
		const location = signal<Catalog.Position | undefined>(undefined)
		this.#signals.effect(() => this.#runLocation(path, location))

		return location.readonly()
	}

	#runLocation(path: string, location: Signal<Catalog.Position | undefined>) {
		cleanup(() => location.set(undefined))

		if (!this.enabled.get()) return

		const broadcast = this.broadcast.get()
		if (!broadcast) return

		const locations = this.catalog.get()?.feedback?.locations
		if (!locations) return

		const track = locations[path]
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
