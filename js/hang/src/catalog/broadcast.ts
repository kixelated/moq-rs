import * as Moq from "@kixelated/moq";
import { z } from "zod";

import { type Audio, AudioSchema } from "./audio";
import { type Location, LocationSchema } from "./location";
import { type Video, VideoSchema } from "./video";

export const BroadcastSchema = z.object({
	video: z.array(VideoSchema).optional(),
	audio: z.array(AudioSchema).optional(),
	location: LocationSchema.optional(),
});

export class Broadcast {
	video: Video[] = [];
	audio: Audio[] = [];
	location?: Location;

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
		broadcast.location = parsed.location;

		return broadcast;
	}

	static async fetch(track: Moq.TrackConsumer): Promise<Broadcast | undefined> {
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
