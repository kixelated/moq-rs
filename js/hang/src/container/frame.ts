import * as Moq from "@kixelated/moq";
import { getVint53, setVint53 } from "./vint";

export interface FrameSource {
	byteLength: number;
	copyTo(buffer: Uint8Array): void;
}

export function encodeFrame(source: Uint8Array | FrameSource, timestamp: number): Uint8Array {
	const data = new Uint8Array(8 + (source instanceof Uint8Array ? source.byteLength : source.byteLength));
	const size = setVint53(data, timestamp).byteLength;
	if (source instanceof Uint8Array) {
		data.set(source, size);
	} else {
		source.copyTo(data.subarray(size));
	}
	return data.subarray(0, (source instanceof Uint8Array ? source.byteLength : source.byteLength) + size);
}

// NOTE: A keyframe is always the first frame in a group, so it's not encoded on the wire.
export function decodeFrame(buffer: Uint8Array): { data: Uint8Array; timestamp: number } {
	const [timestamp, data] = getVint53(buffer);
	return { timestamp, data };
}

export class FrameProducer {
	#track: Moq.TrackProducer;
	#group?: Moq.GroupProducer;

	constructor(track: Moq.TrackProducer) {
		this.#track = track;
	}

	encode(data: Uint8Array | FrameSource, timestamp: number, keyframe: boolean) {
		if (keyframe) {
			this.#group?.close();
			this.#group = this.#track.appendGroup();
		} else if (!this.#group) {
			throw new Error("must start with a keyframe");
		}

		this.#group?.writeFrame(encodeFrame(data, timestamp));
	}

	consume(): FrameConsumer {
		return new FrameConsumer(this.#track.consume());
	}

	close() {
		this.#track.close();
		this.#group?.close();
	}
}

export class FrameConsumer {
	#track: Moq.TrackConsumer;

	constructor(track: Moq.TrackConsumer) {
		this.#track = track;
	}

	async decode(): Promise<{ data: Uint8Array; timestamp: number; keyframe: boolean } | undefined> {
		const next = await this.#track.nextFrame();
		if (!next) return undefined;

		const { timestamp, data } = decodeFrame(next.data);
		return { data, timestamp, keyframe: next.frame === 0 };
	}
}
