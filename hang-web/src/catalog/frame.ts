import type { Group, GroupReader } from "@kixelated/moq";
import { getVint53, setVint53 } from "../util/vint";

export type FrameType = "key" | "delta";

export class Frame {
	type: FrameType;
	timestamp: number;
	data: Uint8Array;

	constructor(type: FrameType, timestamp: number, data: Uint8Array) {
		this.type = type;
		this.timestamp = timestamp;
		this.data = data;
	}

	static async decode(group: GroupReader): Promise<Frame | undefined> {
		const kind = group.index === 0 ? "key" : "delta";
		const payload = await group.readFrame();
		if (!payload) {
			return undefined;
		}

		const [timestamp, data] = getVint53(payload);
		return new Frame(kind, timestamp, data);
	}

	encode(group: Group) {
		if ((group.length === 0) !== (this.type === "key")) {
			throw new Error(`invalid ${this.type} position`);
		}

		let frame = new Uint8Array(8 + this.data.byteLength);
		const size = setVint53(frame, this.timestamp).byteLength;
		frame.set(this.data, size);
		frame = new Uint8Array(frame.buffer, 0, this.data.byteLength + size);

		group.writeFrame(frame);
	}
}
