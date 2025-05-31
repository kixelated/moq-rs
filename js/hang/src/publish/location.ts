import * as Moq from "@kixelated/moq";
import { Derived, Signal, Signals, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";

export type LocationProps = {
	// If true, then we'll publish our position to the broadcast.
	enabled?: boolean;

	// Our initial position.
	position?: Catalog.Position;

	// If true, then this broadcaster allows other peers to request position updates.
	peering?: boolean;
};

export class Location {
	broadcast: Moq.BroadcastProducer;

	enabled: Signal<boolean>;

	position: Signal<Catalog.Position | undefined>;

	peering: Signal<boolean | undefined>;

	#track = new Moq.TrackProducer("location", 0);
	#producer = new LocationProducer(this.#track);

	catalog: Derived<Catalog.Location | undefined>;

	#peers = signal<Record<string, Catalog.Track> | undefined>(undefined);

	#signals = new Signals();

	constructor(broadcast: Moq.BroadcastProducer, props?: LocationProps) {
		this.broadcast = broadcast;

		this.enabled = signal(props?.enabled ?? false);
		this.position = signal(props?.position ?? undefined);
		this.peering = signal(props?.peering ?? undefined);

		broadcast.insertTrack(this.#track.consume());
		this.#signals.cleanup(() => broadcast.removeTrack(this.#track.name));

		this.catalog = this.#signals.derived(() => {
			const enabled = this.enabled.get();
			if (!enabled) return;

			return {
				initial: this.position.peek(), // Doesn't trigger a re-render
				updates: { name: this.#track.name, priority: this.#track.priority },
				peering: this.peering.get(),
				peers: this.#peers.get(),
			};
		});

		this.#signals.effect(() => {
			const position = this.position.get();
			if (!position) return;
			this.#producer.update(position);
		});
	}

	// Request that a peer update their position via their handle.
	peer(handle: string): LocationProducer {
		const track = new Moq.TrackProducer(`location/${handle}`, 0);
		const producer = new LocationProducer(track);
		this.broadcast.insertTrack(track.consume());

		this.#peers.set((prev) => {
			return {
				...(prev ?? {}),
				[handle]: {
					name: track.name,
					priority: track.priority,
				},
			};
		});

		track.closed().then(() => {
			this.broadcast.removeTrack(track.name);

			this.#peers.set((prev) => {
				const { [handle]: _, ...rest } = prev ?? {};
				return {
					...rest,
				};
			});
		});

		return producer;
	}

	close() {
		this.#producer.close();
		this.#signals.close();
	}
}

export class LocationProducer {
	track: Moq.TrackProducer;

	constructor(track: Moq.TrackProducer) {
		this.track = track;
	}

	update(position: Catalog.Position) {
		const group = this.track.appendGroup();

		// We encode everything as JSON for simplicity.
		// In the future, we should encode as a binary format to save bytes.
		const encoder = new TextEncoder();
		const encoded = encoder.encode(JSON.stringify(position));

		console.log("encoded", this.track.name, position);

		group.writeFrame(encoded);
		group.close();
	}

	close() {
		this.track.close();
	}
}
