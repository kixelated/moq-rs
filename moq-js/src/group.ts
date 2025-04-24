export class Group {
	readonly id: number;
	readonly writer: GroupWriter;
	readonly reader: GroupReader;

	constructor(id: number, buffer: number) {
		this.id = id;

		const strategy = new ByteLengthQueuingStrategy({ highWaterMark: buffer });
		const transform = new TransformStream<Uint8Array, Uint8Array>(undefined, strategy, strategy);

		this.writer = new GroupWriter(id, transform.writable);
		this.reader = new GroupReader(id, transform.readable);
	}
}

export class GroupWriter {
	readonly id: number;

	// A stream of frames.
	frames: WritableStream<Uint8Array>;

	constructor(id: number, frames: WritableStream<Uint8Array>) {
		this.id = id;
		this.frames = frames;
	}
}

export class GroupReader {
	readonly id: number;
	frames: ReadableStream<Uint8Array>;

	constructor(id: number, frames: ReadableStream<Uint8Array>) {
		this.id = id;
		this.frames = frames;
	}

	tee(): GroupReader {
		const [one, two] = this.frames.tee();
		this.frames = one;
		return new GroupReader(this.id, two);
	}
}
