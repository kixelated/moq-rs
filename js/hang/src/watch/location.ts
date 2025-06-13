import * as Moq from "@kixelated/moq";
import { Memo, Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";
import * as Container from "../container";

export interface LocationProps {
	enabled?: boolean;
}

export class Location {
	enabled: Signal<boolean>;

	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	catalog: Memo<Catalog.Location | undefined>;
	peering: Memo<boolean | undefined>;

	#current = signal<Catalog.Position | undefined>(undefined);
	readonly current = this.#current.readonly();

	#updates: Memo<Catalog.Track | undefined>;

	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Signal<Catalog.Root | undefined>,
		props?: LocationProps,
	) {
		this.enabled = signal(props?.enabled ?? false);
		this.broadcast = broadcast;

		// Grab the location section from the catalog (if it's changed).
		this.catalog = this.#signals.memo(
			() => {
				if (!this.enabled.get()) return undefined;
				return catalog.get()?.location;
			},
			{ deepEquals: true },
		);
		this.peering = this.#signals.memo(() => this.catalog.get()?.peering);

		this.#signals.effect(() => {
			const catalog = this.catalog.get();
			if (!catalog) return;

			const initial = catalog.initial;
			if (!initial) return;

			this.#current.set(initial);
		});

		this.#updates = this.#signals.memo(
			() => {
				const broadcast = this.broadcast.get();
				if (!broadcast) return;

				const catalog = this.catalog.get();
				if (!catalog) return;

				const updates = catalog.updates;
				if (!updates) return;

				return updates;
			},
			{ deepEquals: true },
		);

		this.#signals.effect(() => {
			const broadcast = this.broadcast.get();
			if (!broadcast) return;

			const updates = this.#updates.get();
			if (!updates) return;

			const track = broadcast.subscribe(updates.name, updates.priority);
			cleanup(() => track.close());

			const consumer = new Container.PositionConsumer(track);
			cleanup(() => consumer.close());

			void runConsumer(consumer, this.#current);
		});
	}

	// Request the location from a specific peer.
	peer(handle?: string): LocationPeer {
		return new LocationPeer(this.broadcast, this.catalog, handle);
	}

	close() {
		this.#signals.close();
	}
}

async function runConsumer(consumer: Container.PositionConsumer, location: Signal<Catalog.Position | undefined>) {
	try {
		for (;;) {
			const position = await consumer.next();
			if (!position) break;

			location.set(position);
		}

		location.set(undefined);
	} catch (err) {
		console.warn("error running location consumer", err);
	} finally {
		consumer.close();
	}
}

export class LocationPeer {
	handle: Signal<string | undefined>;
	location: Signal<Catalog.Position | undefined>;
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;

	#track: Memo<Catalog.Track | undefined>;
	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Memo<Catalog.Location | undefined>,
		handle?: string,
	) {
		this.handle = signal(handle);
		this.location = signal<Catalog.Position | undefined>(undefined);
		this.broadcast = broadcast;

		this.#track = this.#signals.memo(
			() => {
				const handle = this.handle.get();
				if (!handle) return undefined;

				const root = catalog.get();
				if (!root) return undefined;

				const track = root.peers?.[handle];
				if (!track) return undefined;

				return track;
			},
			{ deepEquals: true },
		);

		this.#signals.effect(this.#run.bind(this));
	}

	#run() {
		cleanup(() => this.location.set(undefined));

		const broadcast = this.broadcast.get();
		if (!broadcast) return;

		const track = this.#track.get();
		if (!track) return;

		const sub = broadcast.subscribe(track.name, track.priority);
		cleanup(() => sub.close());

		const consumer = new Container.PositionConsumer(sub);
		void runConsumer(consumer, this.location);
	}

	close() {
		this.#signals.close();
	}
}
