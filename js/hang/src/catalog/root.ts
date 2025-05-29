import * as Moq from "@kixelated/moq"
import { z } from "zod/v4-mini"

import { type Audio, AudioSchema } from "./audio"
import { type Video, VideoSchema } from "./video"
import { Location, LocationSchema } from "./location"

export const RootSchema = z.object({
	video: z.optional(z.array(VideoSchema)),
	audio: z.optional(z.array(AudioSchema)),
	location: z.optional(LocationSchema),
})

export class Root {
	video: Video[] = [];
	audio: Audio[] = [];
	location: Location | undefined

	encode() {
		return JSON.stringify(this)
	}

	static decode(raw: Uint8Array): Root {
		const decoder = new TextDecoder()
		const str = decoder.decode(raw)
		const json = JSON.parse(str)
		const parsed = RootSchema.parse(json)

		const broadcast = new Root()
		broadcast.video = parsed.video ?? []
		broadcast.audio = parsed.audio ?? []
		broadcast.location = parsed.location

		return broadcast
	}

	static async fetch(track: Moq.TrackConsumer): Promise<Root | undefined> {
		const group = await track.nextGroup()
		if (!group) return undefined // track is done

		try {
			const frame = await group.readFrame()
			if (!frame) throw new Error("empty group")
			return Root.decode(frame)
		} finally {
			group.close()
		}
	}
}
