import { Track, TrackReader, TrackWriter } from "./track";
import { Watch, WatchConsumer, WatchProducer } from "./util/async";

export class Broadcast {
	readonly path: string;

	readonly writer: BroadcastWriter;
	readonly reader: BroadcastReader;

	constructor(path: string) {
		this.path = path;

		const watch = new Watch(new State());
		this.writer = new BroadcastWriter(path, watch.producer);
		this.reader = new BroadcastReader(path, watch.consumer);
	}
}

class State {
	tracks = new Map<string, TrackReader>();

	// Called when a track is not found.
	// The receiver is responsible for producing the track.
	onUnknown?: (track: Track) => void;
}

export class BroadcastWriter {
	readonly path: string;
	#state: WatchProducer<State>;

	constructor(path: string, state: WatchProducer<State>) {
		this.path = path;
		this.#state = state;
	}

	create(name: string): TrackWriter {
		const track = new Track(name, 0);
		this.insert(track.reader);
		return track.writer;
	}

	insert(track: TrackReader) {
		this.#state.update((state) => {
			state.tracks.set(track.name, track);
			return state;
		});
	}

	remove(name: string) {
		this.#state.update((state) => {
			state.tracks.delete(name);
			return state;
		});
	}

	unknown(fn?: (track: Track) => void) {
		this.#state.update((state) => {
			state.onUnknown = fn;
			return state;
		});
	}

	async closed(): Promise<void> {
		return this.#state.closed();
	}

	abort(reason: Error) {
		this.#state.abort(reason);
	}

	close() {
		this.#state.update((state) => {
			for (const track of state.tracks.values()) {
				track.close();
			}
			state.tracks.clear();
			return state;
		});
		this.#state.close();
	}
}

export class BroadcastReader {
	readonly path: string;

	#state: WatchConsumer<State>;

	constructor(path: string, state: WatchConsumer<State>) {
		this.path = path;
		this.#state = state;
	}

	subscribe(track: string, priority: number): TrackReader {
		const state = this.#state.latest();

		const existing = state.tracks.get(track);
		if (existing) {
			return existing.clone();
		}

		const pair = new Track(track, priority);

		if (state.onUnknown) {
			state.onUnknown(pair);
		} else {
			pair.writer.abort(new Error("not found"));
		}

		return pair.reader;
	}

	async closed(): Promise<void> {
		return this.#state.closed();
	}

	close() {
		this.#state.close();
	}

	clone(): BroadcastReader {
		return new BroadcastReader(this.path, this.#state.clone());
	}
}
