import { Group, GroupReader, GroupWriter } from "./group";
import { Watch, WatchNext } from "./util/async";
import { Queue, QueueReader, QueueWriter } from "./util/queue";

export class TrackWriter {
	readonly path: string;
	readonly priority: number;

	#group = new Watch<GroupReader | undefined>(undefined);

	constructor(path: string, priority: number) {
		this.path = path;
		this.priority = priority;
	}

	async append(): Promise<GroupWriter> {
		const [latest, _] = this.#group.value();

		const group = new Group(latest ? latest.id + 1 : 0);
		this.#group.update(group.reader);
		return group.writer;
	}

	async insert(group: GroupReader) {
		const [current, _] = this.#group.value();
		if (current && group.id < current.id) {
			throw new Error("old group");
		}

		this.#group.update(group);
	}

	async close() {
		this.#group.close();
	}

	async abort(reason?: unknown) {
		this.#group.abort(reason);
	}
}

export class TrackReader {
	readonly path: string;
	readonly priority: number;

	#groups?: Promise<WatchNext<GroupReader | undefined>>;
	#closed?: Promise<void>;

	constructor(path: string, priority: number, groups: Watch<GroupReader | undefined>) {
		this.path = path;
		this.priority = priority;
		this.#groups = Promise.resolve(groups.value());
		this.#closed = groups.closed();
	}

	async next(): Promise<GroupReader | undefined> {
		if (!this.#groups) return;
		const [current, next] = await this.#groups;
		this.#groups = next;
		return current;
	}

	async closed() {
		return this.#closed;
	}
}
