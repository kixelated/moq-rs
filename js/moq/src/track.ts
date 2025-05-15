import { Group, GroupReader, GroupWriter } from "./group";
import { Watch, WatchConsumer, WatchProducer } from "./util/watch";

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

	close() {
		this.writer.close();
		this.reader.close();
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

	appendGroup(): GroupWriter {
		const group = new Group(this.#next ?? 0);

		this.#next = group.id + 1;
		this.#latest.update((latest) => {
			latest?.close();
			return group.reader;
		});

		return group.writer;
	}

	insertGroup(group: GroupReader) {
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
		try {
			this.#latest.update((latest) => {
				latest?.close();
				return null;
			});

			this.#latest.close();
		} catch {}
	}

	async closed(): Promise<void> {
		await this.#latest.closed();
	}

	abort(reason: Error) {
		try {
			this.#latest.update((latest) => {
				latest?.close();
				return null;
			});
			this.#latest.abort(reason);
		} catch {}
	}
}

export class TrackReader {
	readonly name: string;
	readonly priority: number;

	#groups: WatchConsumer<GroupReader | null>;

	constructor(name: string, priority: number, groups: WatchConsumer<GroupReader | null>) {
		this.name = name;
		this.priority = priority;
		this.#groups = groups;
	}

	async nextGroup(): Promise<GroupReader | undefined> {
		const group = await this.#groups.next((group) => !!group);
		return group?.clone();
	}

	clone(): TrackReader {
		return new TrackReader(this.name, this.priority, this.#groups.clone());
	}

	close() {
		this.#groups.close();
	}

	async closed(): Promise<void> {
		await this.#groups.closed();
	}
}
