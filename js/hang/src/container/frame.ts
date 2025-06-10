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
