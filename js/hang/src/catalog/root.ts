import * as Moq from "@kixelated/moq"
import { z } from "zod/v4-mini"

import { type Audio, AudioSchema } from "./audio"
import { type Video, VideoSchema } from "./video"
import { Location, LocationSchema } from "./location"
import { Feedback, FeedbackSchema } from "./feedback"

export const RootSchema = z.object({
	video: z.optional(z.array(VideoSchema)),
	audio: z.optional(z.array(AudioSchema)),
	location: z.optional(LocationSchema),
	feedback: z.optional(FeedbackSchema),
})

export class Root {
	video: Video[] = [];
	audio: Audio[] = [];
	location: Location | undefined
	feedback: Feedback | undefined

	encode() {
		return JSON.stringify(this)
	}

	static decode(raw: Uint8Array): Root {
		const decoder = new TextDecoder()
		const str = decoder.decode(raw)
		const json = JSON.parse(str)
		const parsed = RootSchema.parse(json)

		const root = new Root()
		root.video = parsed.video ?? []
		root.audio = parsed.audio ?? []
		root.location = parsed.location
		root.feedback = parsed.feedback

		return root
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
