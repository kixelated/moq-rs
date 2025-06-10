import * as Moq from "@kixelated/moq";
import { Memo, Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";

export type LocationProps = {
	// If true, then we'll publish our position to the broadcast.
	enabled?: boolean;

	// Our initial position.
	current?: Catalog.Position;

	// If true, then this broadcaster allows other peers to request position updates.
	peering?: boolean;
};

export class Location {
	broadcast: Moq.BroadcastProducer;

	enabled: Signal<boolean>;

	current: Signal<Catalog.Position | undefined>;
	peering: Signal<boolean | undefined>;

	#track = new Moq.TrackProducer("location.json", 0);
	#producer = new LocationProducer(this.#track);

	catalog: Memo<Catalog.Location | undefined>;

	#peers = signal<Record<string, Catalog.Track> | undefined>(undefined);

	#signals = new Signals();

	constructor(broadcast: Moq.BroadcastProducer, props?: LocationProps) {
		this.broadcast = broadcast;

		this.enabled = signal(props?.enabled ?? false);
		this.current = signal(props?.current ?? undefined);
		this.peering = signal(props?.peering ?? undefined);

		this.catalog = this.#signals.memo(() => {
			const enabled = this.enabled.get();
			if (!enabled) return;

			broadcast.insertTrack(this.#track.consume());
			cleanup(() => broadcast.removeTrack(this.#track.name));

			return {
				initial: this.current.peek(), // Doesn't trigger a re-render
				updates: { name: this.#track.name, priority: this.#track.priority },
				peering: this.peering.get(),
				peers: this.#peers.get(),
			};
		});

		this.#signals.effect(() => {
			const position = this.current.get();
			if (!position) return;
			this.#producer.update(position);
		});
	}

	// Request that a peer update their position via their handle.
	peer(handle?: string): LocationPeer {
		return new LocationPeer(this.broadcast, this.#peers, handle);
	}

	close() {
		this.#producer.close();
		this.#signals.close();
	}
}

export class LocationPeer {
	handle: Signal<string | undefined>;
	catalog: Signal<Record<string, Catalog.Track> | undefined>;
	broadcast: Moq.BroadcastProducer;
	//location: Signal<Catalog.Position | undefined>
	producer: Memo<LocationProducer | undefined>;

	#signals = new Signals();

	constructor(
		broadcast: Moq.BroadcastProducer,
		catalog: Signal<Record<string, Catalog.Track> | undefined>,
		handle?: string,
	) {
		this.handle = signal(handle);
		this.catalog = catalog;
		this.broadcast = broadcast;

		this.producer = this.#signals.memo(() => {
			const handle = this.handle.get();
			if (!handle) return;

			const track = new Moq.TrackProducer(`peer/${handle}/location.json`, 0);
			cleanup(() => track.close());

			broadcast.insertTrack(track.consume());
			cleanup(() => broadcast.removeTrack(track.name));

			this.catalog.set((prev) => {
				return {
					...(prev ?? {}),
					[handle]: {
						name: track.name,
						priority: track.priority,
					},
				};
			});

			cleanup(() => {
				this.catalog.set((prev) => {
					const { [handle]: _, ...rest } = prev ?? {};
					return {
						...rest,
					};
				});
			});

			const producer = new LocationProducer(track);
			cleanup(() => producer.close());

			return producer;
		});
	}

	close() {
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

		group.writeFrame(encoded);
		group.close();
	}

	close() {
		this.track.close();
	}
}
