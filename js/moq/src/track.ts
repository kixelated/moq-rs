import { GroupConsumer, GroupProducer } from "./group";
import { WatchConsumer, WatchProducer } from "./util/watch";

/**
 * Handles writing and managing groups in a track.
 *
 * @public
 */
export class TrackProducer {
	/** The name of the track */
	readonly name: string;
	/** The priority level of the track */
	readonly priority: number;

	#latest = new WatchProducer<GroupConsumer | null>(null);
	#next?: number;

	/**
	 * Creates a new TrackProducer with the specified name, priority, and latest group producer.
	 * @param name - The name of the track
	 * @param priority - The priority level
	 * @param latest - The latest group producer
	 *
	 * @internal
	 */
	constructor(name: string, priority?: number) {
		this.name = name;
		this.priority = priority ?? 0;
	}

	/**
	 * Appends a new group to the track.
	 * @returns A GroupProducer for the new group
	 */
	appendGroup(): GroupProducer {
		const group = new GroupProducer(this.#next ?? 0);

		this.#next = group.id + 1;
		this.#latest.update((latest) => {
			latest?.close();
			return group.consume();
		});

		return group;
	}

	/**
	 * Inserts an existing group into the track.
	 * @param group - The group to insert
	 */
	insertGroup(group: GroupConsumer) {
		if (group.id < (this.#next ?? 0)) {
			group.close();
			return;
		}

		this.#next = group.id + 1;
		this.#latest.update((latest) => {
			latest?.close();
			return group;
		});
	}

	/**
	 * Closes the publisher and all associated groups.
	 */
	close() {
		try {
			this.#latest.update((latest) => {
				latest?.close();
				return null;
			});

			this.#latest.close();
		} catch {}
	}

	closed(): Promise<void> {
		return this.#latest.closed();
	}

	/**
	 * Returns a promise that resolves when the publisher is unused.
	 * @returns A promise that resolves when unused
	 */
	async unused(): Promise<void> {
		await this.#latest.unused();
	}

	consume(): TrackConsumer {
		return new TrackConsumer(this.name, this.priority, this.#latest.consume());
	}

	/**
	 * Aborts the publisher with an error.
	 * @param reason - The error reason for aborting
	 */
	abort(reason: Error) {
		try {
			this.#latest.update((latest) => {
				latest?.close();
				return null;
			});
			this.#latest.abort(reason);
		} catch {}
	}
}

/**
 * Handles reading groups from a track.
 *
 * @public
 */
export class TrackConsumer {
	/** The name of the track */
	readonly name: string;
	/** The priority level of the track */
	readonly priority: number;

	#groups: WatchConsumer<GroupConsumer | null>;

	// State used for the nextFrame helper.
	#currentGroup?: GroupConsumer;
	#currentFrame = 0;

	#nextGroup?: Promise<GroupConsumer | undefined>;
	#nextFrame?: Promise<Uint8Array | undefined>;

	/**
	 * Creates a new TrackConsumer with the specified name, priority, and groups consumer.
	 * @param name - The name of the track
	 * @param priority - The priority level
	 * @param groups - The groups consumer
	 *
	 * @internal
	 */
	constructor(name: string, priority: number, groups: WatchConsumer<GroupConsumer | null>) {
		this.name = name;
		this.priority = priority;
		this.#groups = groups;

		// Start fetching the next group immediately.
		this.#nextGroup = this.#groups.next((group) => !!group).then((group) => group?.clone());
	}

	/**
	 * Gets the next group from the track.
	 * @returns A promise that resolves to the next group or undefined
	 */
	async nextGroup(): Promise<GroupConsumer | undefined> {
		const group = await this.#nextGroup;

		// Start fetching the next group immediately.
		this.#nextGroup = this.#groups.next((group) => !!group).then((group) => group?.clone());

		// Update the state needed for the nextFrame() helper.
		this.#currentGroup?.close();
		this.#currentGroup = group?.clone(); // clone so we don't steal from the returned consumer
		this.#currentFrame = 0;
		this.#nextFrame = this.#currentGroup?.nextFrame();

		return group;
	}

	/**
	 * A helper that returns the next frame in group order, skipping old groups/frames if needed.
	 *
	 * Returns the data and the index of the frame/group.
	 */
	async nextFrame(): Promise<{ group: number; frame: number; data: Uint8Array } | undefined> {
		for (;;) {
			const next = await this.#next();
			if (!next) return undefined;

			if ("frame" in next) {
				if (!this.#currentGroup) {
					throw new Error("impossible");
				}

				// Start reading the next frame.
				this.#nextFrame = this.#currentGroup?.nextFrame();

				// Return the frame and increment the frame index.
				return { group: this.#currentGroup?.id, frame: this.#currentFrame++, data: next.frame };
			}

			this.#nextGroup = this.#groups.next((group) => !!group).then((group) => group?.clone());

			if (this.#currentGroup && this.#currentGroup.id >= next.group.id) {
				// Skip this old group.
				next.group.close();
				continue;
			}

			// Skip the rest of the current group.
			this.#currentGroup?.close();
			this.#currentGroup = next.group;
			this.#currentFrame = 0;

			// Start reading the next frame.
			this.#nextFrame = this.#currentGroup?.nextFrame();
		}
	}

	// Returns the next non-undefined value from the nextFrame or nextGroup promises.
	async #next(): Promise<{ frame: Uint8Array } | { group: GroupConsumer } | undefined> {
		const nextFrame = this.#nextFrame
			?.then((frame) => ({ frame }))
			// Ignore errors reading the group; we can move on to the next group instead.
			.catch((error) => {
				console.warn("ignoring error reading frame", error);
				return { frame: undefined };
			}) ?? { frame: undefined };

		const nextGroup = this.#nextGroup?.then((group) => ({ group })) ?? { group: undefined };

		// The order matters here, because Promise.race returns the first resolved value *in order*.
		// This is also why we're not using Promise.any, because I think it works differently?
		const result = await Promise.race([nextFrame, nextGroup]);
		if ("frame" in result) {
			if (result.frame) {
				return { frame: result.frame };
			}

			const other = await nextGroup;
			return other.group ? { group: other.group } : undefined;
		}

		if (result.group) {
			return { group: result.group };
		}

		const other = await nextFrame;
		return other.frame ? { frame: other.frame } : undefined;
	}

	/**
	 * Creates a new instance of the consumer using the same groups consumer.
	 *
	 * The current group and position within the group is not preserved.
	 *
	 * @returns A new TrackConsumer instance
	 */
	clone(): TrackConsumer {
		return new TrackConsumer(this.name, this.priority, this.#groups.clone());
	}

	/**
	 * Closes the consumer.
	 */
	close() {
		this.#groups.close();

		this.#nextGroup?.then((group) => group?.close());
		this.#nextGroup = undefined;
		this.#nextFrame = undefined;
		this.#currentGroup?.close();
		this.#currentGroup = undefined;
	}

	/**
	 * Returns a promise that resolves when the consumer is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		await this.#groups.closed();
	}
}
