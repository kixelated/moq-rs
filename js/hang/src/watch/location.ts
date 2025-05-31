import * as Moq from "@kixelated/moq";
import { Memo, Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";

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
		this.catalog = this.#signals.memo(() => {
			return this.enabled.get() ? catalog.get()?.location : undefined;
		});
		this.peering = this.#signals.memo(() => this.catalog.get()?.peering);

		// Use equals to prevent re-subscribing to an identical track.
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
			{ equals: (a, b) => a?.name === b?.name },
		);

		this.#signals.effect(() => {
			const broadcast = this.broadcast.get();
			if (!broadcast) return;

			const updates = this.#updates.get();
			if (!updates) return;

			const track = broadcast.subscribe(updates.name, updates.priority);
			cleanup(() => track.close());

			const consumer = new LocationConsumer(track);
			void runConsumer(consumer, this.#current);

			cleanup(() => consumer.close());
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

async function runConsumer(consumer: LocationConsumer, location: Signal<Catalog.Position | undefined>) {
	for (;;) {
		const position = await consumer.next();
		if (!position) break;

		location.set(position);
	}

	location.set(undefined);
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

		this.#track = this.#signals.memo(() => {
			const handle = this.handle.get();
			if (!handle) return undefined;

			const root = catalog.get();
			if (!root) return undefined;

			const track = root.peers?.[handle];
			if (!track) return undefined;

			return track;
		});

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

		const consumer = new LocationConsumer(sub);
		void runConsumer(consumer, this.location);
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

			return position;
		} finally {
			group.close();
		}
	}

	close() {
		this.track.close();
	}
}
