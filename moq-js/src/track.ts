import { Group, GroupReader, GroupWriter } from "./group";
import { WatchConsumer, WatchProducer } from "./util/async";

export class Track {
	readonly name: string;
	readonly priority: number;

	readonly writer: TrackWriter;
	readonly reader: TrackReader;

	constructor(name: string, priority: number) {
		this.name = name;
		this.priority = priority;

		const [producer, consumer] = WatchProducer.pair<GroupReader>();
		this.writer = new TrackWriter(name, priority, producer);
		this.reader = new TrackReader(name, priority, consumer);
	}
}

export class TrackWriter {
	readonly name: string;
	readonly priority: number;

	#group: WatchProducer<GroupReader>;

	constructor(name: string, priority: number, group: WatchProducer<GroupReader>) {
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

	abort(reason?: unknown) {
		this.#group.abort(reason);
	}
}

export class TrackReader {
	readonly name: string;
	readonly priority: number;

	#group: WatchConsumer<GroupReader>;

	constructor(name: string, priority: number, group: WatchConsumer<GroupReader>) {
		this.name = name;
		this.priority = priority;
		this.#group = group;
	}

	async nextGroup(): Promise<GroupReader | undefined> {
		return await this.#group.next();
	}

	tee(): TrackReader {
		return new TrackReader(this.name, this.priority, this.#group.clone());
	}

	close() {
		this.#group.close();
	}
}
