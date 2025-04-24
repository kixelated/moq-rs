export type Announcement = {
	broadcast: string;
	active: boolean;
};

export class Announced {
	readonly prefix: string;
	readonly writer: AnnouncedWriter;
	readonly reader: AnnouncedReader;

	constructor(prefix: string) {
		this.prefix = prefix;

		// We don't really want backpressure here, so use a value that should never be hit.
		const backpressure = { highWaterMark: 1024 };

		const queue = new TransformStream<Announcement, Announcement>(undefined, backpressure, backpressure);
		this.writer = new AnnouncedWriter(prefix, queue.writable);
		this.reader = new AnnouncedReader(prefix, queue.readable);
	}
}

export class AnnouncedWriter {
	readonly broadcast: string;
	#queue: WritableStream<Announcement>;

	constructor(broadcast: string, queue: WritableStream<Announcement>) {
		this.broadcast = broadcast;
		this.#queue = queue;
	}

	async write(announcement: Announcement) {
		const writer = this.#queue.getWriter();
		await writer.write(announcement);
		writer.releaseLock();
	}

	async abort(reason?: unknown) {
		await this.#queue.abort(reason);
	}

	async close() {
		await this.#queue.close();
	}
}

export class AnnouncedReader {
	readonly broadcast: string;
	#queue: ReadableStream<Announcement>;

	constructor(broadcast: string, queue: ReadableStream<Announcement>) {
		this.broadcast = broadcast;
		this.#queue = queue;
	}

	async read(): Promise<Announcement | undefined> {
		const reader = this.#queue.getReader();
		const result = await reader.read();
		reader.releaseLock();
		return result.done ? undefined : result.value;
	}

	async close() {
		await this.#queue.cancel();
	}

	tee(): AnnouncedReader {
		const [one, two] = this.#queue.tee();
		this.#queue = one;
		return new AnnouncedReader(this.broadcast, two);
	}
}
