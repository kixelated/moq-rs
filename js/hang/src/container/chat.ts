import * as Moq from "@kixelated/moq";

export class ChatProducer {
	#track: Moq.TrackProducer;

	constructor(track: Moq.TrackProducer) {
		this.#track = track;
	}

	// Chat messages are just text/markdown.
	encode(markdown: string) {
		const encoder = new TextEncoder();
		const buffer = encoder.encode(markdown);

		const group = this.#track.appendGroup();
		group.writeFrame(buffer);
		group.close();
	}

	consume(): ChatConsumer {
		return new ChatConsumer(this.#track.consume());
	}

	close() {
		this.#track.close();
	}
}

export class ChatConsumer {
	#track: Moq.TrackConsumer;

	constructor(track: Moq.TrackConsumer) {
		this.#track = track;
	}

	// Chat messages are just text/markdown.
	async next(): Promise<string | undefined> {
		const frame = await this.#track.nextFrame();
		if (!frame) return undefined;

		const decoder = new TextDecoder();
		return decoder.decode(frame.data);
	}

	close() {
		this.#track.close();
	}
}
