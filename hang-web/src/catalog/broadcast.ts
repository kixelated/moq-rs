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

	static async fetch(connection: Moq.Connection, path: string): Promise<Broadcast> {
		const track = new Moq.TrackWriter(path, 0);
		console.debug("fetching catalog:", path);

		const sub = await connection.subscribe(track);
		try {
			const segment = await sub.nextGroup();
			if (!segment) throw new Error("no catalog data");

			const frame = await segment.nextFrame();
			if (!frame) throw new Error("no catalog frame");

			segment.close();

			const broadcast = Broadcast.decode(frame);

			console.debug("decoded catalog:", broadcast);
			return broadcast;
		} finally {
			sub.close();
		}
	}
}
