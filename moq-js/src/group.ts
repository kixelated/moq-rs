export class Group {
	readonly id: number;
	readonly writer: GroupWriter;
	readonly reader: GroupReader;

	constructor(id: number) {
		this.id = id;

		// arbitrary 32MB limit
		const strategy = new ByteLengthQueuingStrategy({ highWaterMark: 1024 * 1024 * 32 });
		const transform = new TransformStream<Uint8Array, Uint8Array>(undefined, strategy, strategy);

		this.writer = new GroupWriter(id, transform.writable);
		this.reader = new GroupReader(id, transform.readable);
	}
}

export class GroupWriter {
	readonly id: number;

	// A stream of frames.
	#frames: WritableStreamDefaultWriter<Uint8Array>;

	constructor(id: number, frames: WritableStream<Uint8Array>) {
		this.id = id;
		this.#frames = frames.getWriter();
	}

	async writeFrame(frame: Uint8Array) {
		await this.#frames.write(frame);
	}

	close() {
		this.#frames.close().catch(() => {});
	}

	abort(reason?: unknown) {
		this.#frames.abort(reason).catch(() => {});
	}
}

export class GroupReader {
	readonly id: number;
	#frames: ReadableStream<Uint8Array>;

	constructor(id: number, frames: ReadableStream<Uint8Array>) {
		this.id = id;
		this.#frames = frames;
	}

	async readFrame(): Promise<Uint8Array | undefined> {
		const reader = this.#frames.getReader();
		const result = await reader.read();
		reader.releaseLock();
		return result.done ? undefined : result.value;
	}

	close() {
		this.#frames.cancel().catch(() => {});
	}

	clone(): GroupReader {
		const [one, two] = this.#frames.tee();
		this.#frames = one;
		return new GroupReader(this.id, two);
	}
}
