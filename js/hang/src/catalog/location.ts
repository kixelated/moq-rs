import { z } from "zod/v4-mini"
import { TrackSchema } from "./track"

export const PositionSchema = z.object({
	x: z.number(),
	y: z.number(),
})

export const LocationSchema = z.object({
	initial: PositionSchema,
	track: z.optional(TrackSchema),
})

export type Location = z.infer<typeof LocationSchema>
export type Position = z.infer<typeof PositionSchema>