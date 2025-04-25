import { z } from "zod";

import * as Moq from "@kixelated/moq";

import { type Audio, AudioSchema } from "./audio";
import { type Video, VideoSchema } from "./video";

export const BroadcastSchema = z.object({
	video: z.array(VideoSchema).optional(),
	audio: z.array(AudioSchema).optional(),
});

export class Broadcast {
	video: Video[] = [];
	audio: Audio[] = [];

	encode() {
		return JSON.stringify(this);
	}

	static decode(raw: Uint8Array): Broadcast {
		const decoder = new TextDecoder();
		const str = decoder.decode(raw);
		const json = JSON.parse(str);
		const parsed = BroadcastSchema.parse(json);

		const broadcast = new Broadcast();
		broadcast.video = parsed.video ?? [];
		broadcast.audio = parsed.audio ?? [];
		return broadcast;
	}

	static async fetch(track: Moq.TrackReader): Promise<Broadcast | undefined> {
		const group = await track.nextGroup();
		if (!group) return undefined; // track is done

		try {
			const frame = await group.readFrame();
			if (!frame) throw new Error("empty group");
			return Broadcast.decode(frame);
		} finally {
			group.close();
		}
	}
}
