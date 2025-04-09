// A 1:1 queue, allowing one writer and one reader.
export class Queue<T> {
	readonly reader: QueueReader<T>;
	readonly writer: QueueWriter<T>;

	constructor(capacity = 1) {
		const queue = new CountQueuingStrategy({ highWaterMark: capacity });
		const stream = new TransformStream({}, undefined, queue);
		this.reader = new QueueReader(stream.readable);
		this.writer = new QueueWriter(stream.writable);
	}
}

export class QueueWriter<T> {
	#writer: WritableStream<T>;

	constructor(writer: WritableStream<T>) {
		this.#writer = writer;
	}

	async write(v: T) {
		const writer = this.#writer.getWriter();
		await writer.write(v);
		writer.releaseLock();
	}

	async close() {
		await this.#writer.close();
	}

	async abort(reason?: unknown) {
		await this.#writer.abort(reason);
	}
}

export class QueueReader<T> {
	#reader: ReadableStream<T>;

	constructor(reader: ReadableStream<T>) {
		this.#reader = reader;
	}

	async read(): Promise<T | undefined> {
		const reader = this.#reader.getReader();
		const { value, done } = await reader.read();
		reader.releaseLock();

		if (done) return;
		return value;
	}

	async cancel(reason?: unknown) {
		await this.#reader.cancel(reason);
	}

	async closed() {
		const reader = this.#reader.getReader();
		const closed = reader.closed;
		reader.releaseLock();
		await closed;
	}

	// Split the reader into two independent readers.
	tee(): [QueueReader<T>, QueueReader<T>] {
		const [one, two] = this.#reader.tee();
		return [new QueueReader(one), new QueueReader(two)];
	}
}
