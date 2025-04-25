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

	#group: WatchProducer<GroupReader | null>;

	constructor(name: string, priority: number, group: WatchProducer<GroupReader | null>) {
		this.name = name;
		this.priority = priority;
		this.#group = group;
	}

	appendGroup(): GroupWriter {
		const latest = this.#group.latest();
		const group = new Group(latest ? latest.id + 1 : 0);
		this.insertGroup(group.reader);
		return group.writer;
	}

	insertGroup(group: GroupReader) {
		const latest = this.#group.latest();
		if (latest) {
			// Skip any old groups.
			if (group.id < latest.id) {
				group.close();
				return;
			}

			// Close the previous group.
			latest.close();
		}

		this.#group.update(group);
	}

	close() {
		this.#group.close();
	}

	async closed(): Promise<void> {
		await this.#group.closed();
	}

	abort(reason?: unknown) {
		this.#group.abort(reason);
	}
}

export class TrackReader {
	readonly name: string;
	readonly priority: number;

	#group: WatchConsumer<GroupReader | null>;

	constructor(name: string, priority: number, group: WatchConsumer<GroupReader | null>) {
		this.name = name;
		this.priority = priority;
		this.#group = group;
	}

	async nextGroup(): Promise<GroupReader | undefined> {
		for (;;) {
			const group = await this.#group.next();
			if (group === null) continue; // skip the first null
			return group;
		}
	}

	clone(): TrackReader {
		return new TrackReader(this.name, this.priority, this.#group.clone());
	}

	close() {
		this.#group.close();
	}
}
