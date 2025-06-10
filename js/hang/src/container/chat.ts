import * as Moq from "@kixelated/moq";
import { Decoder, DecoderProps } from "./decoder";

export type ChatMessage = {
	timestamp: DOMHighResTimeStamp;
	text: string;
};

// A wrapper around Decoder that converts messages to text.
export class ChatDecoder {
	#decoder: Decoder;

	constructor(props?: DecoderProps) {
		this.#decoder = new Decoder(props);
	}

	get track(): Moq.TrackConsumer | undefined {
		return this.#decoder.track;
	}

	set track(track: Moq.TrackConsumer | undefined) {
		this.#decoder.track = track;
	}

	get epoch(): number {
		return this.#decoder.epoch;
	}

	set epoch(epoch: number) {
		this.#decoder.epoch = epoch;
	}

	async readMessage(): Promise<ChatMessage | undefined> {
		const frame = await this.#decoder.readFrame();
		if (!frame) return;

		const text = new TextDecoder().decode(frame.data);
		console.debug("received chat frame", text);
		return { timestamp: frame.timestamp, text };
	}

	close() {
		this.#decoder.close();
	}
}
