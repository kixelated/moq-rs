import type { GroupReader, GroupWriter } from "@kixelated/moq";
import { getVint53, setVint53 } from "../util/vint";

export class Frame {
	keyframe: boolean;
	timestamp: number;
	data: Uint8Array;

	constructor(keyframe: boolean, timestamp: number, data: Uint8Array) {
		this.keyframe = keyframe;
		this.timestamp = timestamp;
		this.data = data;
	}

	static async decode(group: GroupReader, keyframe: boolean): Promise<Frame | undefined> {
		const payload = await group.readFrame();
		if (!payload) {
			return undefined;
		}

		const [timestamp, data] = getVint53(payload);
		return new Frame(keyframe, timestamp, data);
	}

	encode(group: GroupWriter) {
		let frame = new Uint8Array(8 + this.data.byteLength);
		const size = setVint53(frame, this.timestamp).byteLength;
		frame.set(this.data, size);
		frame = new Uint8Array(frame.buffer, 0, this.data.byteLength + size);

		group.writeFrame(frame);
	}
}
