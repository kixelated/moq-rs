import { GroupReader, GroupWriter } from "./group";
import { Watch } from "./util/async";
import { Closed } from "./util/error";

export class TrackWriter {
	readonly path: string;
	readonly priority: number;

	// TODO use an array
	latest = new Watch<GroupReader | undefined>(undefined);

	readers = 0;
	closed: Closed | null = null;

	constructor(path: string, priority: number) {
		this.path = path;
		this.priority = priority;
	}

	appendGroup(): GroupWriter {
		const next = this.latest.value()[0]?.id ?? 0;
		return this.createGroup(next);
	}

	createGroup(sequence: number): GroupWriter {
		if (this.closed) throw this.closed;

		const group = new GroupWriter(sequence);
		const [current, _] = this.latest.value();

		// TODO use an array
		if (!current || current.id < sequence) {
			const reader = new GroupReader(group);
			this.latest.update(reader);
		}

		return group;
	}

	close(closed?: Closed) {
		if (this.closed) return;
		this.closed = closed ?? new Closed();
		this.latest.close();
	}

	reader(): TrackReader {
		// VERY important that readers are closed to decrement the count
		this.readers += 1;
		return new TrackReader(this);
	}
}

export class TrackReader {
	latest: number | null = null;
	#track: TrackWriter;

	constructor(track: TrackWriter) {
		this.#track = track;
	}

	async nextGroup(): Promise<GroupReader | undefined> {
		let [current, next] = this.#track.latest.value();

		for (;;) {
			if (current && this.latest !== current.id) {
				this.latest = current.id;
				return current;
			}

			if (this.#track.closed) throw this.#track.closed;

			if (!next) return;
			[current, next] = await next;
		}
	}

	get path() {
		return this.#track.path;
	}

	get priority() {
		return this.#track.priority;
	}

	close() {
		this.#track.readers -= 1;
		if (this.#track.readers <= 0) this.#track.close();
	}
}
