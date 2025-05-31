import * as Moq from "@kixelated/moq"
import { z } from "zod/v4-mini"

import { AudioSchema } from "./audio"
import { VideoSchema } from "./video"
import { LocationSchema } from "./location"
import { CapabilitiesSchema } from "./capabilities"

export const RootSchema = z.object({
	video: z.optional(z.array(VideoSchema)),
	audio: z.optional(z.array(AudioSchema)),
	location: z.optional(LocationSchema),
	capabilities: z.optional(CapabilitiesSchema),
})

export type Root = z.infer<typeof RootSchema>

export function encode(root: Root): Uint8Array {
	const encoder = new TextEncoder()
	return encoder.encode(JSON.stringify(root))
}

export function decode(raw: Uint8Array): Root {
	const decoder = new TextDecoder()
	const str = decoder.decode(raw)
	try {
		const json = JSON.parse(str)
		return RootSchema.parse(json)
	} catch (error) {
		console.error("invalid catalog", str)
		throw error
	}
}

export async function fetch(track: Moq.TrackConsumer): Promise<Root | undefined> {
	const group = await track.nextGroup()
	if (!group) return undefined // track is done

	try {
		const frame = await group.readFrame()
		if (!frame) throw new Error("empty group")
		return decode(frame)
	} finally {
		group.close()
	}
}
