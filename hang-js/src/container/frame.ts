import type { Group, GroupReader } from "@kixelated/moq";
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

	static async decode(group: GroupReader): Promise<Frame | undefined> {
		const keyframe = group.index === 0;
		const payload = await group.nextFrame();
		if (!payload) {
			return undefined;
		}

		const [timestamp, data] = getVint53(payload);
		return new Frame(keyframe, timestamp, data);
	}

	encode(group: Group) {
		if ((group.length === 0) !== this.keyframe) {
			throw new Error("keyframe must start a group");
		}

		let frame = new Uint8Array(8 + this.data.byteLength);
		const size = setVint53(frame, this.timestamp).byteLength;
		frame.set(this.data, size);
		frame = new Uint8Array(frame.buffer, 0, this.data.byteLength + size);

		group.writeFrame(frame);
	}
}
