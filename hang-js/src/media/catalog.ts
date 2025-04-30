import { z } from "zod";

import * as Moq from "@kixelated/moq";

import { type Audio, AudioSchema } from "./audio";
import { type Video, VideoSchema } from "./video";
import { type Location, LocationSchema } from "./location";

export const CatalogSchema = z.object({
	video: z.array(VideoSchema).optional(),
	audio: z.array(AudioSchema).optional(),
	location: LocationSchema.optional(),
});

export class Catalog {
	video: Video[] = [];
	audio: Audio[] = [];
	location?: Location;

	encode() {
		return JSON.stringify(this);
	}

	static decode(raw: Uint8Array): Catalog {
		const decoder = new TextDecoder();
		const str = decoder.decode(raw);
		const json = JSON.parse(str);
		const parsed = CatalogSchema.parse(json);

		const broadcast = new Catalog();
		broadcast.video = parsed.video ?? [];
		broadcast.audio = parsed.audio ?? [];
		broadcast.location = parsed.location;

		return broadcast;
	}

	static async fetch(track: Moq.TrackReader): Promise<Catalog | undefined> {
		const group = await track.nextGroup();
		if (!group) return undefined; // track is done

		try {
			const frame = await group.readFrame();
			if (!frame) throw new Error("empty group");
			return Catalog.decode(frame);
		} finally {
			group.close();
		}
	}
}
