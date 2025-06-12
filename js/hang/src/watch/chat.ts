import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";

import { Memo, Signal, Signals, cleanup, signal } from "@kixelated/signals";
import { Container } from "..";

export interface ChatProps {
	// Whether to start downloading the chat.
	// Defaults to false so you can make sure everything is ready before starting.
	enabled?: boolean;
}

export class Chat {
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;

	enabled: Signal<boolean>;
	catalog: Memo<Catalog.Chat | undefined>;
	track: Memo<Container.ChatConsumer | undefined>;
	ttl: Memo<DOMHighResTimeStamp | undefined>;

	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Signal<Catalog.Root | undefined>,
		props?: ChatProps,
	) {
		this.broadcast = broadcast;
		this.enabled = signal(props?.enabled ?? false);

		// Grab the chat section from the catalog (if it's changed).
		this.catalog = this.#signals.memo(
			() => {
				if (!this.enabled.get()) return undefined;
				return catalog.get()?.chat;
			},
			{ deepEquals: true },
		);

		// TODO enforce the TTL?
		this.ttl = this.#signals.memo(() => {
			return this.catalog.get()?.ttl;
		});

		this.track = this.#signals.memo(() => {
			const catalog = this.catalog.get();
			if (!catalog) return undefined;

			const broadcast = this.broadcast.get();
			if (!broadcast) return undefined;

			const track = broadcast.subscribe(catalog.track.name, catalog.track.priority);
			const consumer = new Container.ChatConsumer(track);

			cleanup(() => consumer.close());
			return consumer;
		});
	}

	close() {
		this.#signals.close();
	}
}
