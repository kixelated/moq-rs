import { Watch, WatchConsumer, WatchProducer } from "./util/watch";

export class Group {
	readonly id: number;
	readonly writer: GroupWriter;
	readonly reader: GroupReader;

	constructor(id: number) {
		this.id = id;

		const watch = new Watch<Uint8Array[]>([]);

		this.writer = new GroupWriter(id, watch.producer);
		this.reader = new GroupReader(id, watch.consumer);
	}
}

export class GroupWriter {
	readonly id: number;

	// A stream of frames.
	#frames: WatchProducer<Uint8Array[]>;

	constructor(id: number, frames: WatchProducer<Uint8Array[]>) {
		this.id = id;
		this.#frames = frames;
	}

	writeFrame(frame: Uint8Array) {
		this.#frames.update((frames) => [...frames, frame]);
	}

	close() {
		this.#frames.close();
	}

	async closed(): Promise<void> {
		await this.#frames.closed();
	}

	abort(reason: Error) {
		this.#frames.abort(reason);
	}
}

export class GroupReader {
	readonly id: number;

	#frames: WatchConsumer<Uint8Array[]>;
	#index = 0;

	constructor(id: number, frames: WatchConsumer<Uint8Array[]>) {
		this.id = id;
		this.#frames = frames;
	}

	async readFrame(): Promise<Uint8Array | undefined> {
		const frames = await this.#frames.when((frames) => frames.length > this.#index);
		return frames?.at(this.#index++);
	}

	close() {
		this.#frames.close();
	}

	clone(): GroupReader {
		return new GroupReader(this.id, this.#frames.clone());
	}
}
