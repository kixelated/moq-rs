import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";

import { cleanup, Memo, Signal, signal, Signals } from "@kixelated/signals";
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

	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Signal<Catalog.Root | undefined>,
		props?: ChatProps,
	) {
		this.broadcast = broadcast;
		this.enabled = signal(props?.enabled ?? false);

		this.catalog = this.#signals.memo(() => {
			return this.enabled.get() ? catalog.get()?.chat : undefined;
		});
	}

	consume(): Container.ChatDecoder {
		// This works because Decoder supports signals... kinda.
		const decoder = new Container.ChatDecoder();

		this.#signals.effect(() => {
			const catalog = this.catalog.get();
			if (!catalog) return;

			const broadcast = this.broadcast.get();
			if (!broadcast) return;

			const track = broadcast.subscribe(catalog.track.name, catalog.track.priority);
			decoder.track = track;
			decoder.epoch = catalog.epoch ?? 0;

			cleanup(() => {
				track.close();
			});
		});

		return decoder;
	}

	close() {
		this.#signals.close();
	}
}
