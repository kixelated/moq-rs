import { TrackConsumer, TrackProducer } from "./track";
import { WatchConsumer, WatchProducer } from "./util/watch";

class State {
	tracks = new Map<string, TrackConsumer>();

	// Called when a track is not found.
	// The receiver is responsible for producing the track.
	onUnknown?: (track: TrackProducer) => void;
}

/**
 * Handles writing and managing tracks in a broadcast.
 *
 * @public
 */
export class BroadcastProducer {
	/** The path identifier for this broadcast */
	#state: WatchProducer<State>;

	/**
	 * Creates a new BroadcastProducer with the specified path and state.
	 * @param state - The state producer
	 *
	 * @internal
	 */
	constructor() {
		this.#state = new WatchProducer<State>(new State());
	}

	/**
	 * Creates a new track with the specified name.
	 * @param name - The name of the track to create
	 * @returns A TrackProducer for the new track
	 */
	createTrack(name: string): TrackProducer {
		const track = new TrackProducer(name, 0);
		this.insertTrack(track.consume());
		return track;
	}

	/**
	 * Inserts an existing track into the broadcast.
	 * @param track - The track reader to insert
	 */
	insertTrack(track: TrackConsumer) {
		this.#state.update((state) => {
			const existing = state.tracks.get(track.name);
			existing?.close();
			state.tracks.set(track.name, track);
			return state;
		});
	}

	/**
	 * Removes a track from the broadcast.
	 * @param name - The name of the track to remove
	 */
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

	/**
	 * Sets a callback for handling unknown (on-demand) tracks.
	 * If not specified, unknown tracks will be closed with a "not found" error.
	 *
	 * @param fn - The callback function to handle unknown tracks
	 */
	unknownTrack(fn?: (track: TrackProducer) => void) {
		this.#state.update((state) => {
			state.onUnknown = fn;
			return state;
		});
	}

	/**
	 * Aborts the writer with an error.
	 * @param reason - The error reason for aborting
	 */
	abort(reason: Error) {
		this.#state.abort(reason);
	}

	/**
	 * Closes the writer and all associated tracks.
	 */
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

	/**
	 * Returns a promise that resolves when the writer is unused.
	 */
	async unused(): Promise<void> {
		return this.#state.unused();
	}

	consume(): BroadcastConsumer {
		return new BroadcastConsumer(this.#state.consume());
	}
}

/**
 * Handles reading and subscribing to tracks in a broadcast.
 *
 * @remarks `clone()` can be used to create multiple consumers, just remember to `close()` them.
 *
 * @public
 */
export class BroadcastConsumer {
	#state: WatchConsumer<State>;

	/**
	 * Creates a new BroadcastConsumer with the specified path and state.
	 * @param path - The path identifier
	 * @param state - The state consumer
	 *
	 * @internal
	 */
	constructor(state: WatchConsumer<State>) {
		this.#state = state;
	}

	/**
	 * Subscribes to a track with the specified priority.
	 * @param track - The name of the track to subscribe to
	 * @param priority - The priority level for the subscription
	 * @returns A TrackConsumer for the subscribed track
	 */
	subscribe(track: string, priority: number): TrackConsumer {
		const state = this.#state.value();

		const existing = state.tracks.get(track);
		if (existing) {
			return existing.clone();
		}

		const producer = new TrackProducer(track, priority);
		const consumer = producer.consume();

		if (state.onUnknown) {
			state.onUnknown(producer);
		} else {
			producer.abort(new Error("not found"));
		}

		return consumer;
	}

	/**
	 * Returns a promise that resolves when the reader is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		return this.#state.closed();
	}

	/**
	 * Closes the reader.
	 */
	close() {
		this.#state.close();
	}

	/**
	 * Creates a new instance of the reader using the same state.
	 * @returns A new BroadcastConsumer instance
	 */
	clone(): BroadcastConsumer {
		return new BroadcastConsumer(this.#state.clone());
	}
}
