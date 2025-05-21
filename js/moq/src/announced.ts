import { WatchConsumer, WatchProducer } from "./util/watch";

/**
 * The availability of a broadcast.
 *
 * @public
 */
export interface Announce {
	path: string;
	active: boolean;
}

/**
 * Handles writing announcements to the announcement queue.
 *
 * @public
 */
export class AnnouncedProducer {
	#queue = new WatchProducer<Announce[]>([]);

	/**
	 * Writes an announcement to the queue.
	 * @param announcement - The announcement to write
	 */
	write(announcement: Announce) {
		this.#queue.update((announcements) => {
			announcements.push(announcement);
			return announcements;
		});
	}

	/**
	 * Aborts the writer with an error.
	 * @param reason - The error reason for aborting
	 */
	abort(reason: Error) {
		this.#queue.abort(reason);
	}

	/**
	 * Closes the writer.
	 */
	close() {
		this.#queue.close();
	}

	/**
	 * Returns a promise that resolves when the writer is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		await this.#queue.closed();
	}

	/**
	 * Creates a new AnnouncedConsumer that only returns the announcements for the specified prefix.
	 * @param prefix - The prefix for the consumer
	 * @returns A new AnnouncedConsumer instance
	 */
	consume(prefix = ""): AnnouncedConsumer {
		return new AnnouncedConsumer(this.#queue.consume(), prefix);
	}
}

/**
 * Handles reading announcements from the announcement queue.
 *
 * @public
 */
export class AnnouncedConsumer {
	/** The prefix for this reader */
	readonly prefix: string;

	#queue: WatchConsumer<Announce[]>;
	#index = 0;

	/**
	 * Creates a new AnnounceConsumer with the specified prefix and queue.
	 * @param prefix - The prefix for the reader
	 * @param queue - The queue to read announcements from
	 *
	 * @internal
	 */
	constructor(queue: WatchConsumer<Announce[]>, prefix = "") {
		this.#queue = queue;
		this.prefix = prefix;
	}

	/**
	 * Returns the next announcement from the queue.
	 * @returns A promise that resolves to the next announcement or undefined
	 */
	async next(): Promise<Announce | undefined> {
		for (;;) {
			const queue = await this.#queue.when((v) => v.length > this.#index);
			if (!queue) return undefined;

			while (this.#index < queue.length) {
				const announce = queue.at(this.#index++);
				if (!announce?.path.startsWith(this.prefix)) continue;

				// We have to remove the prefix so we only return our suffix.
				return {
					path: announce.path.slice(this.prefix.length),
					active: announce.active,
				};
			}
		}
	}

	/**
	 * Closes the reader.
	 */
	close() {
		this.#queue.close();
	}

	/**
	 * Returns a promise that resolves when the reader is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		await this.#queue.closed();
	}

	/**
	 * Creates a new instance of the reader using the same queue and prefix.
	 *
	 * @returns A new AnnounceConsumer instance
	 */
	clone(): AnnouncedConsumer {
		return new AnnouncedConsumer(this.#queue.clone(), this.prefix);
	}
}
