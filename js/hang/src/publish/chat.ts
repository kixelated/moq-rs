import { signal, Signal, Signals, Memo, cleanup } from "@kixelated/signals";
import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Container from "../container";

export type ChatProps = {
	enabled?: boolean;
};

export class Chat {
	broadcast: Moq.BroadcastProducer;
	enabled: Signal<boolean>;

	catalog: Memo<Catalog.Chat | undefined>;

	// Always create the track, even if we're not publishing it
	#track = new Moq.TrackProducer("chat.md", 0);
	#group = this.#track.appendGroup();
	#epoch = performance.now();

	#signals = new Signals();

	constructor(broadcast: Moq.BroadcastProducer, props?: ChatProps) {
		this.broadcast = broadcast;
		this.enabled = signal(props?.enabled ?? false);

		this.catalog = this.#signals.memo(() => {
			const enabled = this.enabled.get();
			if (!enabled) return;

			broadcast.insertTrack(this.#track.consume());
			cleanup(() => broadcast.removeTrack(this.#track.name));

			return { track: { name: this.#track.name, priority: this.#track.priority } };
		});
	}

	// Publish a message to the chat, using the current time as the timestamp.
	publish(text: string) {
		// Convert the text to a buffer
		const encoder = new TextEncoder();
		const buffer = encoder.encode(text);
		const timestamp = performance.now() - this.#epoch;

		const frame = Container.encodeFrame(buffer, timestamp);
		console.debug("published chat frame", text);
		this.#group.writeFrame(frame);
	}

	// Optionally consume our published messages for local playback.
	consume(): Container.ChatDecoder {
		return new Container.ChatDecoder({ track: this.#track.consume(), epoch: this.#epoch });
	}

	clear() {
		this.#group.close();

		// We create a new group with no frames to uncache the previous group.
		this.#group = this.#track.appendGroup();
	}

	close() {
		this.#group.close();
		this.#signals.close();
	}
}
