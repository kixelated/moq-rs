import { GroupConsumer, GroupProducer } from "./group"
import { WatchConsumer, WatchProducer } from "./util/watch"

/**
 * Handles writing and managing groups in a track.
 *
 * @public
 */
export class TrackProducer {
	/** The name of the track */
	readonly name: string
	/** The priority level of the track */
	readonly priority: number

	#latest = new WatchProducer<GroupConsumer | null>(null);
	#next?: number

	/**
	 * Creates a new TrackProducer with the specified name, priority, and latest group producer.
	 * @param name - The name of the track
	 * @param priority - The priority level
	 * @param latest - The latest group producer
	 *
	 * @internal
	 */
	constructor(name: string, priority: number) {
		this.name = name
		this.priority = priority
	}

	/**
	 * Appends a new group to the track.
	 * @returns A GroupProducer for the new group
	 */
	appendGroup(): GroupProducer {
		const group = new GroupProducer(this.#next ?? 0)

		this.#next = group.id + 1
		this.#latest.update((latest) => {
			latest?.close()
			return group.consume()
		})

		return group
	}

	/**
	 * Inserts an existing group into the track.
	 * @param group - The group to insert
	 */
	insertGroup(group: GroupConsumer) {
		if (group.id < (this.#next ?? 0)) {
			group.close()
			return
		}

		this.#next = group.id + 1
		this.#latest.update((latest) => {
			latest?.close()
			return group
		})
	}

	/**
	 * Closes the publisher and all associated groups.
	 */
	close() {
		try {
			this.#latest.update((latest) => {
				latest?.close()
				return null
			})

			this.#latest.close()
		} catch { }
	}

	closed(): Promise<void> {
		return this.#latest.closed()
	}

	/**
	 * Returns a promise that resolves when the publisher is unused.
	 * @returns A promise that resolves when unused
	 */
	async unused(): Promise<void> {
		await this.#latest.unused()
	}

	consume(): TrackConsumer {
		return new TrackConsumer(this.name, this.priority, this.#latest.consume())
	}

	/**
	 * Aborts the publisher with an error.
	 * @param reason - The error reason for aborting
	 */
	abort(reason: Error) {
		try {
			this.#latest.update((latest) => {
				latest?.close()
				return null
			})
			this.#latest.abort(reason)
		} catch { }
	}
}

/**
 * Handles reading groups from a track.
 *
 * @public
 */
export class TrackConsumer {
	/** The name of the track */
	readonly name: string
	/** The priority level of the track */
	readonly priority: number

	#groups: WatchConsumer<GroupConsumer | null>

	/**
	 * Creates a new TrackConsumer with the specified name, priority, and groups consumer.
	 * @param name - The name of the track
	 * @param priority - The priority level
	 * @param groups - The groups consumer
	 *
	 * @internal
	 */
	constructor(name: string, priority: number, groups: WatchConsumer<GroupConsumer | null>) {
		this.name = name
		this.priority = priority
		this.#groups = groups
	}

	/**
	 * Gets the next group from the track.
	 * @returns A promise that resolves to the next group or undefined
	 */
	async nextGroup(): Promise<GroupConsumer | undefined> {
		const group = await this.#groups.next((group) => !!group)
		return group?.clone()
	}

	/**
	 * Creates a new instance of the consumer using the same groups consumer.
	 * @returns A new TrackConsumer instance
	 */
	clone(): TrackConsumer {
		return new TrackConsumer(this.name, this.priority, this.#groups.clone())
	}

	/**
	 * Closes the consumer.
	 */
	close() {
		this.#groups.close()
	}

	/**
	 * Returns a promise that resolves when the consumer is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		await this.#groups.closed()
	}
}
