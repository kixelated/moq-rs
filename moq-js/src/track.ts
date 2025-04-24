import { Group, GroupReader, GroupWriter } from "./group";
import { Watch, WatchNext } from "./util/async";

export class Track {
	readonly name: string;
	readonly priority: number;

	readonly writer: TrackWriter;
	readonly reader: TrackReader;

	constructor(name: string, priority: number) {
		this.name = name;
		this.priority = priority;

		// We don't really want backpressure, so put something that should never be hit.
		const backpressure = { highWaterMark: 32 };
		const group = new TransformStream<GroupReader, GroupReader>(undefined, backpressure, backpressure);
		this.writer = new TrackWriter(name, priority, group.writable);
		this.reader = new TrackReader(name, priority, group.readable);
	}
}

export class TrackWriter {
	readonly name: string;
	readonly priority: number;

	#group: WritableStream<GroupReader>;
	#latest: number | undefined;

	constructor(name: string, priority: number, group: WritableStream<GroupReader>) {
		this.name = name;
		this.priority = priority;
		this.#group = group;
	}

	async append(buffer: number): Promise<GroupWriter> {
		const group = new Group(this.#latest ? this.#latest + 1 : 0, buffer);
		this.#latest = group.id;

		// TODO avoid this backpressure; we only care about the latest group.
		const writer = this.#group.getWriter();
		await writer.write(group.reader);
		writer.releaseLock();

		return group.writer;
	}

	async insert(group: GroupReader) {
		if (this.#latest && group.id < this.#latest) {
			return;
		}

		this.#latest = group.id;

		// TODO avoid this backpressure; we only care about the latest group.
		const writer = this.#group.getWriter();
		await writer.write(group);
		writer.releaseLock();
	}

	async close() {
		await this.#group.close();
	}

	async abort(reason?: unknown) {
		await this.#group.abort(reason);
	}
}

export class TrackReader {
	readonly name: string;
	readonly priority: number;

	#group: ReadableStream<GroupReader>;

	constructor(name: string, priority: number, group: ReadableStream<GroupReader>) {
		this.name = name;
		this.priority = priority;
		this.#group = group;
	}

	async next(): Promise<GroupReader | undefined> {
		const reader = this.#group.getReader();
		const result = await reader.read();
		reader.releaseLock();
		return result.done ? undefined : result.value;
	}

	tee(): TrackReader {
		const [one, two] = this.#group.tee();
		this.#group = one;
		return new TrackReader(this.name, this.priority, two);
	}

	async close() {
		await this.#group.cancel();
	}
}
