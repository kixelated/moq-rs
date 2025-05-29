import { z } from "zod/v4-mini"
import { TrackSchema } from "./track"

export const PositionSchema = z.object({
	x: z.number(),
	y: z.number(),
})

export const LocationSchema = z.object({
	initial: z.optional(PositionSchema),
	track: z.optional(TrackSchema),
})

// As part of feedback, viewers can advertise the location of another broadcaster.
// This is opt-in, and the broadcaster can choose to ignore the feedback.
export const LocationsSchema = z.record(z.string(), TrackSchema)

export type Location = z.infer<typeof LocationSchema>
export type Position = z.infer<typeof PositionSchema>
export type Locations = z.infer<typeof LocationsSchema>