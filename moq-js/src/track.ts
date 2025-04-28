import { Group, GroupReader, GroupWriter } from "./group";

export class Track {
	readonly name: string;
	readonly priority: number;

	readonly writer: TrackWriter;
	readonly reader: TrackReader;

	constructor(name: string, priority: number) {
		this.name = name;
		this.priority = priority;

		const backpressure = new CountQueuingStrategy({ highWaterMark: 8 });
		const watch = new TransformStream<GroupReader, GroupReader>(undefined, backpressure, backpressure);
		this.writer = new TrackWriter(name, priority, watch.writable);
		this.reader = new TrackReader(name, priority, watch.readable);
	}
}

export class TrackWriter {
	readonly name: string;
	readonly priority: number;

	#groups: WritableStreamDefaultWriter<GroupReader | null>;
	#next?: number;

	constructor(name: string, priority: number, groups: WritableStream<GroupReader | null>) {
		this.name = name;
		this.priority = priority;
		this.#groups = groups.getWriter();
	}

	async appendGroup(): Promise<GroupWriter> {
		const group = new Group(this.#next ?? 0);
		this.#next = group.id + 1;
		await this.#groups.write(group.reader);
		return group.writer;
	}

	async insertGroup(group: GroupReader) {
		this.#next = Math.max(this.#next ?? 0, group.id + 1);
		await this.#groups.write(group);
	}

	close() {
		this.#groups.close().catch(() => {});
	}

	async closed(): Promise<void> {
		await this.#groups.closed;
	}

	abort(reason?: unknown) {
		this.#groups.abort(reason).catch(() => {});
	}
}

export class TrackReader {
	readonly name: string;
	readonly priority: number;

	#groups: ReadableStream<GroupReader>;
	#reader: ReadableStreamDefaultReader<GroupReader>;
	#latest?: GroupReader;

	constructor(name: string, priority: number, groups: ReadableStream<GroupReader>) {
		this.name = name;
		this.priority = priority;
		this.#groups = groups;
		this.#reader = groups.getReader();
	}

	async nextGroup(): Promise<GroupReader | undefined> {
		const group = await this.#reader.read();
		this.#latest = group.value;
		return group.value;
	}

	// NOTE: can't be called while `nextGroup` is running.
	clone(): TrackReader {
		this.#reader.releaseLock();

		const [one, two] = this.#groups.tee();
		this.#groups = one;
		this.#reader = one.getReader();

		const latest = this.#latest?.clone();

		// Create a new reader that starts with `this.#latest` and clones each group.
		const cloner = new TransformStream<GroupReader, GroupReader>({
			start(controller) {
				if (latest) {
					controller.enqueue(latest);
				}
			},
			transform(chunk, controller) {
				controller.enqueue(chunk.clone());
			},
		});

		return new TrackReader(this.name, this.priority, two.pipeThrough(cloner));
	}

	close() {
		this.#reader.cancel().catch(() => {});
	}
}
