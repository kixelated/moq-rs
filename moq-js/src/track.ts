import { Group, GroupReader, GroupWriter } from "./group";
import { Watch, WatchConsumer, WatchProducer } from "./util/async";

export class Track {
	readonly name: string;
	readonly priority: number;

	readonly writer: TrackWriter;
	readonly reader: TrackReader;

	constructor(name: string, priority: number) {
		this.name = name;
		this.priority = priority;

		const watch = new Watch<GroupReader | null>(null);
		this.writer = new TrackWriter(name, priority, watch.producer);
		this.reader = new TrackReader(name, priority, watch.consumer);
	}
}

export class TrackWriter {
	readonly name: string;
	readonly priority: number;

	#latest: WatchProducer<GroupReader | null>;
	#next?: number;

	constructor(name: string, priority: number, latest: WatchProducer<GroupReader | null>) {
		this.name = name;
		this.priority = priority;
		this.#latest = latest;
	}

	append(): GroupWriter {
		const group = new Group(this.#next ?? 0);

		this.#next = group.id + 1;
		this.#latest.update((latest) => {
			if (latest) {
				latest.close();
			}
			return group.reader;
		});
		return group.writer;
	}

	insert(group: GroupReader) {
		if (group.id < (this.#next ?? 0)) {
			group.close();
			return;
		}

		this.#next = group.id + 1;
		this.#latest.update((latest) => {
			latest?.close();
			return group;
		});
	}

	close() {
		this.#latest.close();
	}

	async closed(): Promise<void> {
		await this.#latest.closed();
	}

	abort(reason: Error) {
		this.#latest.abort(reason);
	}
}

export class TrackReader {
	readonly name: string;
	readonly priority: number;

	#latest: WatchConsumer<GroupReader | null>;

	constructor(name: string, priority: number, latest: WatchConsumer<GroupReader | null>) {
		this.name = name;
		this.priority = priority;
		this.#latest = latest;
	}

	async next(): Promise<GroupReader | undefined> {
		let group = await this.#latest.next();
		if (group === null) {
			// First call only.
			group = await this.#latest.next();
		}

		return group?.clone();
	}

	clone(): TrackReader {
		return new TrackReader(this.name, this.priority, this.#latest.clone());
	}

	close() {
		this.#latest.close();
	}

	async closed(): Promise<void> {
		await this.#latest.closed();
	}
}
