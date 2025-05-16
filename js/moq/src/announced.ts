import { Watch, WatchConsumer, WatchProducer } from "./util/watch";

/**
 * The availability of a broadcast.
 *
 * @public
 */
export type Announce = {
	broadcast: string;
	active: boolean;
};

/**
 * A Writer/Reader pair for producing and consuming announcements.
 *
 * @public
 */
export class Announced {
	/** The prefix for all announcements managed by this instance */
	readonly prefix: string;
	/** The writer component for creating announcements */
	readonly producer: AnnouncedProducer;
	/** The reader component for consuming announcements */
	readonly consumer: AnnouncedConsumer;

	/**
	 * Creates a new Announced instance with the specified prefix.
	 * @param prefix - The string prefix.
	 */
	constructor(prefix: string) {
		this.prefix = prefix;

		// TODO This grows unbounded. We should remove ended broadcasts.
		const queue = new Watch<Announce[]>([]);
		this.producer = new AnnouncedProducer(prefix, queue.producer);
		this.consumer = new AnnouncedConsumer(prefix, queue.consumer);
	}

	/**
	 * Closes both the writer and reader components.
	 */
	close() {
		this.producer.close();
		this.consumer.close();
	}

	abort(reason: Error) {
		this.producer.abort(reason);
		this.consumer.close();
	}
}

/**
 * Handles writing announcements to the announcement queue.
 *
 * @public
 */
export class AnnouncedProducer {
	/** The broadcast identifier for this writer */
	readonly broadcast: string;
	#queue: WatchProducer<Announce[]>;

	/**
	 * Creates a new AnnounceProducer with the specified broadcast and queue.
	 *
	 * @param broadcast - The broadcast identifier
	 * @param queue - The queue to write announcements to
	 *
	 * @internal
	 */
	constructor(broadcast: string, queue: WatchProducer<Announce[]>) {
		this.broadcast = broadcast;
		this.#queue = queue;
	}

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
	constructor(prefix: string, queue: WatchConsumer<Announce[]>) {
		this.prefix = prefix;
		this.#queue = queue;
	}

	/**
	 * Returns the next announcement from the queue.
	 * @returns A promise that resolves to the next announcement or undefined
	 */
	async next(): Promise<Announce | undefined> {
		const queue = await this.#queue.when((v) => v.length > this.#index);
		return queue?.at(this.#index++);
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
	 * Creates a new instance of the reader using the same queue.
	 * @returns A new AnnounceConsumer instance
	 */
	clone(): AnnouncedConsumer {
		return new AnnouncedConsumer(this.prefix, this.#queue.clone());
	}
}
