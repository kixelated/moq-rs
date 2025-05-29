import { z } from "zod/v4-mini"

export const TrackSchema = z.object({
	name: z.string(),
	priority: z.uint32(), // TODO u8
})

export type Track = z.infer<typeof TrackSchema>
