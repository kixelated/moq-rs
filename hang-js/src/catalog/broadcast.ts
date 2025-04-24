import { z } from "zod";

import * as Moq from "@kixelated/moq";

import { type Audio, AudioSchema } from "./audio";
import { type Video, VideoSchema } from "./video";

export const BroadcastSchema = z.object({
	video: z.array(VideoSchema),
	audio: z.array(AudioSchema),
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
		console.log("decoding catalog:", json);
		const parsed = BroadcastSchema.parse(json);

		const broadcast = new Broadcast();
		broadcast.video = parsed.video;
		broadcast.audio = parsed.audio;
		return broadcast;
	}

	static async fetch(track: Moq.TrackReader): Promise<Broadcast> {
		try {
			const segment = await track.nextGroup();
			if (!segment) throw new Error("no catalog data");

			const frame = await segment.readFrame();
			if (!frame) throw new Error("no catalog frame");

			await segment.close();

			const broadcast = Broadcast.decode(frame);

			console.debug("decoded catalog:", broadcast);
			return broadcast;
		} finally {
			track.close();
		}
	}
}
