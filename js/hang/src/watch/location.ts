import * as Moq from "@kixelated/moq";
import { Derived, Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";

export interface LocationProps {
	enabled?: boolean;
}

export class Location {
	enabled: Signal<boolean>;

	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	catalog: Derived<Catalog.Location | undefined>;
	peering: Derived<boolean | undefined>;

	#current = signal<Catalog.Position | undefined>(undefined);
	readonly current = this.#current.readonly();

	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Signal<Catalog.Root | undefined>,
		props?: LocationProps,
	) {
		this.enabled = signal(props?.enabled ?? false);
		this.broadcast = broadcast;
		this.catalog = this.#signals.derived(() => {
			return this.enabled.get() ? catalog.get()?.location : undefined;
		});
		this.peering = this.#signals.derived(() => this.catalog.get()?.peering);

		this.#signals.effect(this.#run.bind(this));
	}

	#run() {
		const broadcast = this.broadcast.get();
		if (!broadcast) return;

		const catalog = this.catalog.get();
		if (!catalog) return;

		this.#current.set(catalog.initial);
		cleanup(() => this.#current.set(undefined));

		const updates = catalog.updates;
		if (!updates) return;

		const track = broadcast.subscribe(updates.name, updates.priority);
		cleanup(() => track.close());

		const consumer = new LocationConsumer(track);
		void this.#runConsumer(consumer, this.#current);

		cleanup(() => consumer.close());
	}

	// Request a reactive signal for a specific handle on demand.
	// This is useful when publishing, as you only want to subscribe to the feedback you need.
	// TODO: This API is super gross and leaks. We should figure out a better way to do this.
	peer(handle: Signal<string>): Derived<Catalog.Position | undefined> {
		const location = signal<Catalog.Position | undefined>(undefined);
		this.#signals.effect(() => this.#runPeer(handle, location));

		return location.readonly();
	}

	#runPeer(handle: Signal<string>, location: Signal<Catalog.Position | undefined>) {
		cleanup(() => location.set(undefined));

		const broadcast = this.broadcast.get();
		if (!broadcast) return;

		const catalog = this.catalog.get();
		if (!catalog) return;

		const path = handle.get();
		if (!path) return;

		const track = catalog.peers?.[path];
		if (!track) return;

		const sub = broadcast.subscribe(track.name, track.priority);
		cleanup(() => sub.close());

		const consumer = new LocationConsumer(sub);
		void this.#runConsumer(consumer, location);
	}

	async #runConsumer(consumer: LocationConsumer, location: Signal<Catalog.Position | undefined>) {
		for (;;) {
			const position = await consumer.next();
			if (!position) break;

			location.set(position);
		}

		location.set(undefined);
	}

	close() {
		this.#signals.close();
	}
}

export class LocationConsumer {
	track: Moq.TrackConsumer;

	constructor(track: Moq.TrackConsumer) {
		this.track = track;
	}

	async next(): Promise<Catalog.Position | undefined> {
		const group = await this.track.nextGroup();
		if (!group) return undefined;

		try {
			const frame = await group.readFrame();
			if (!frame) return undefined;

			const decoder = new TextDecoder();
			const str = decoder.decode(frame);
			const position = Catalog.PositionSchema.parse(JSON.parse(str));
			console.log("decoded", this.track.name, position);

			return position;
		} finally {
			group.close();
		}
	}

	close() {
		this.track.close();
	}
}
