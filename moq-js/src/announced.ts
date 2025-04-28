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
	#queue: WritableStreamDefaultWriter<Announcement>;

	constructor(broadcast: string, queue: WritableStream<Announcement>) {
		this.broadcast = broadcast;
		this.#queue = queue.getWriter();
	}

	async write(announcement: Announcement) {
		await this.#queue.write(announcement);
	}

	abort(reason?: unknown) {
		this.#queue.abort(reason).catch(() => {});
	}

	close() {
		this.#queue.close().catch(() => {});
		this.#queue.releaseLock();
	}
}

export class AnnouncedReader {
	readonly prefix: string;
	#queue: ReadableStream<Announcement>;

	constructor(broadcast: string, queue: ReadableStream<Announcement>) {
		this.prefix = broadcast;
		this.#queue = queue;
	}

	async next(): Promise<Announcement | undefined> {
		const reader = this.#queue.getReader();
		const result = await reader.read();
		reader.releaseLock();
		return result.done ? undefined : result.value;
	}

	close() {
		this.#queue.cancel().catch(() => {});
	}

	clone(): AnnouncedReader {
		const [one, two] = this.#queue.tee();
		this.#queue = one;
		return new AnnouncedReader(this.prefix, two);
	}
}
