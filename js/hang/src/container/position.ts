import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";

// TODO Make a binary format for this.

export class PositionProducer {
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

export class PositionConsumer {
	track: Moq.TrackConsumer;

	constructor(track: Moq.TrackConsumer) {
		this.track = track;
	}

	async next(): Promise<Catalog.Position | undefined> {
		const frame = await this.track.nextFrame();
		if (!frame) return undefined;

		const decoder = new TextDecoder();
		const str = decoder.decode(frame.data);
		const position = Catalog.PositionSchema.parse(JSON.parse(str));

		return position;
	}

	close() {
		this.track.close();
	}
}
