import { Signal, signal } from "@kixelated/signals"

import * as Catalog from "../catalog"
import * as Moq from "@kixelated/moq"
import { LocationProducer } from "./location"

export interface FeedbackProps {
	enabled?: boolean
}

// This is feedback that we are publishing.
export class Feedback {
	broadcast: Moq.BroadcastProducer
	enabled: Signal<boolean>

	#catalog = signal<Catalog.Feedback | undefined>(undefined)
	readonly catalog = this.#catalog.readonly()

	constructor(broadcast: Moq.BroadcastProducer, props?: FeedbackProps) {
		this.broadcast = broadcast
		this.enabled = signal(props?.enabled ?? false)
	}

	location(path: string): LocationProducer {
		const track = new Moq.TrackProducer(`location/${path}`, 0)
		const producer = new LocationProducer(track)
		this.broadcast.insertTrack(track.consume())

		this.#catalog.set((prev) => {
			return {
				...(prev?.locations ?? {}),
				[path]: {
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
				const { [path]: _, ...rest } = prev?.locations ?? {}
				return {
					...prev,
					locations: rest,
				}
			})
		})

		return producer
	}
}
