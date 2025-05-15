import { Watch, WatchConsumer, WatchProducer } from "./util/watch";

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

		// TODO This grows unbounded. We should remove ended broadcasts.
		const queue = new Watch<Announcement[]>([]);
		this.writer = new AnnouncedWriter(prefix, queue.producer);
		this.reader = new AnnouncedReader(prefix, queue.consumer);
	}

	close() {
		this.writer.close();
		this.reader.close();
	}
}

export class AnnouncedWriter {
	readonly broadcast: string;
	#queue: WatchProducer<Announcement[]>;

	constructor(broadcast: string, queue: WatchProducer<Announcement[]>) {
		this.broadcast = broadcast;
		this.#queue = queue;
	}

	write(announcement: Announcement) {
		this.#queue.update((announcements) => {
			announcements.push(announcement);
			return announcements;
		});
	}

	abort(reason: Error) {
		this.#queue.abort(reason);
	}

	close() {
		this.#queue.close();
	}

	async closed(): Promise<void> {
		await this.#queue.closed();
	}
}

export class AnnouncedReader {
	readonly prefix: string;

	#queue: WatchConsumer<Announcement[]>;
	#index = 0;

	constructor(prefix: string, queue: WatchConsumer<Announcement[]>) {
		this.prefix = prefix;
		this.#queue = queue;
	}

	async next(): Promise<Announcement | undefined> {
		const queue = await this.#queue.when((v) => v.length > this.#index);
		return queue?.at(this.#index++);
	}

	close() {
		this.#queue.close();
	}

	async closed(): Promise<void> {
		await this.#queue.closed();
	}

	clone(): AnnouncedReader {
		return new AnnouncedReader(this.prefix, this.#queue.clone());
	}
}
