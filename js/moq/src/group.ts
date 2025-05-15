import { Watch, WatchConsumer, WatchProducer } from "./util/watch";

/**
 * Represents a group of frames with a writer/reader pair.
 *
 * @public
 */
export class Group {
	/** The unique identifier for this group */
	readonly id: number;
	/** The writer component for adding frames to the group */
	readonly producer: GroupProducer;
	/** The reader component for consuming frames from the group */
	readonly consumer: GroupConsumer;

	/**
	 * Creates a new Group instance with the specified ID.
	 * @param id - The unique identifier for the group
	 */
	constructor(id: number) {
		this.id = id;

		const watch = new Watch<Uint8Array[]>([]);

		this.producer = new GroupProducer(id, watch.producer);
		this.consumer = new GroupConsumer(id, watch.consumer);
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
 * Handles writing frames to a group.
 *
 * @public
 */
export class GroupProducer {
	/** The unique identifier for this writer */
	readonly id: number;

	// A stream of frames.
	#frames: WatchProducer<Uint8Array[]>;

	/**
	 * Creates a new GroupProducer with the specified ID and frames producer.
	 * @param id - The unique identifier
	 * @param frames - The frames producer
	 *
	 * @internal
	 */
	constructor(id: number, frames: WatchProducer<Uint8Array[]>) {
		this.id = id;
		this.#frames = frames;
	}

	/**
	 * Writes a frame to the group.
	 * @param frame - The frame to write
	 */
	writeFrame(frame: Uint8Array) {
		this.#frames.update((frames) => [...frames, frame]);
	}

	/**
	 * Closes the writer.
	 */
	close() {
		this.#frames.close();
	}

	/**
	 * Returns a promise that resolves when the writer is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		await this.#frames.closed();
	}

	/**
	 * Aborts the writer with an error.
	 * @param reason - The error reason for aborting
	 */
	abort(reason: Error) {
		this.#frames.abort(reason);
	}
}

/**
 * Handles reading frames from a group.
 *
 * @public
 */
export class GroupConsumer {
	/** The unique identifier for this reader */
	readonly id: number;

	#frames: WatchConsumer<Uint8Array[]>;
	#index = 0;

	/**
	 * Creates a new GroupConsumer with the specified ID and frames consumer.
	 * @param id - The unique identifier
	 * @param frames - The frames consumer
	 *
	 * @internal
	 */
	constructor(id: number, frames: WatchConsumer<Uint8Array[]>) {
		this.id = id;
		this.#frames = frames;
	}

	/**
	 * Reads the next frame from the group.
	 * @returns A promise that resolves to the next frame or undefined
	 */
	async readFrame(): Promise<Uint8Array | undefined> {
		const frames = await this.#frames.when((frames) => frames.length > this.#index);
		return frames?.at(this.#index++);
	}

	/**
	 * Closes the reader.
	 */
	close() {
		this.#frames.close();
	}

	/**
	 * Creates a new instance of the reader using the same frames consumer.
	 * @returns A new GroupConsumer instance
	 */
	clone(): GroupConsumer {
		return new GroupConsumer(this.id, this.#frames.clone());
	}
}
