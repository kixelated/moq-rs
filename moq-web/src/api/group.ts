import { Watch } from "./util/async";
import { Closed } from "./util/error";

export class Group {
	readonly id: number;

	chunks = new Watch<Uint8Array[]>([]);
	readers = 0;
	closed: Closed | null = null;

	constructor(id: number) {
		this.id = id;
	}

	writeFrame(frame: Uint8Array) {
		if (this.closed) throw this.closed;
		this.chunks.update((chunks) => [...chunks, frame]);
	}

	writeFrames(...frames: Uint8Array[]) {
		if (this.closed) throw this.closed;
		this.chunks.update((chunks) => [...chunks, ...frames]);
		this.close();
	}

	reader(): GroupReader {
		this.readers += 1;
		return new GroupReader(this);
	}

	get length(): number {
		return this.chunks.value()[0].length;
	}

	close(closed?: Closed) {
		if (this.closed) return;
		this.closed = closed ?? new Closed();
		this.chunks.close();
	}
}

export class GroupReader {
	#group: Group;
	#index = 0;

	constructor(group: Group) {
		this.#group = group;
	}

	async nextFrame(): Promise<Uint8Array | undefined> {
		let [chunks, next] = this.#group.chunks.value();

		for (;;) {
			if (this.#index < chunks.length) {
				this.#index += 1;
				return chunks[this.#index - 1];
			}

			if (this.#group.closed) {
				throw this.#group.closed;
			}

			if (!next) return;
			[chunks, next] = await next;
		}
	}

	get index(): number {
		return this.#index;
	}

	get id(): number {
		return this.#group.id;
	}

	close() {
		this.#group.readers -= 1;
		if (this.#group.readers <= 0) this.#group.close();
	}
}
