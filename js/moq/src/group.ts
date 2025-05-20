import { WatchConsumer, WatchProducer } from "./util/watch";

/**
 * Handles writing frames to a group.
 *
 * @public
 */
export class GroupProducer {
	/** The unique identifier for this writer */
	readonly id: number;

	// A stream of frames.
	#frames = new WatchProducer<Uint8Array[]>([]);

	/**
	 * Creates a new GroupProducer with the specified ID and frames producer.
	 * @param id - The unique identifier
	 *
	 * @internal
	 */
	constructor(id: number) {
		this.id = id;
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
	 * Returns a promise that resolves when the writer is unused.
	 * @returns A promise that resolves when unused
	 */
	async unused(): Promise<void> {
		await this.#frames.unused();
	}

	/**
	 * Aborts the writer with an error.
	 * @param reason - The error reason for aborting
	 */
	abort(reason: Error) {
		this.#frames.abort(reason);
	}

	consume(): GroupConsumer {
		return new GroupConsumer(this.#frames.consume(), this.id);
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
	constructor(frames: WatchConsumer<Uint8Array[]>, id: number) {
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
		return new GroupConsumer(this.#frames.clone(), this.id);
	}
}
