import { Track, TrackReader, TrackWriter } from "./track";
import { Watch, WatchConsumer, WatchProducer } from "./util/watch";

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

	close() {
		this.writer.close();
		this.reader.close();
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

	createTrack(name: string): TrackWriter {
		const track = new Track(name, 0);
		this.insertTrack(track.reader);
		return track.writer;
	}

	insertTrack(track: TrackReader) {
		this.#state.update((state) => {
			const existing = state.tracks.get(track.name);
			existing?.close();
			state.tracks.set(track.name, track);
			return state;
		});
	}

	removeTrack(name: string) {
		try {
			this.#state.update((state) => {
				const track = state.tracks.get(name);
				track?.close();
				state.tracks.delete(name);
				return state;
			});
		} catch {
			// We don't care if we try to remove when closed
		}
	}

	unknownTrack(fn?: (track: Track) => void) {
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
		const state = this.#state.value();

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
