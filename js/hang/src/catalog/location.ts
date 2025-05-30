import { z } from "zod/v4-mini"
import { TrackSchema } from "./track"

export const PositionSchema = z.object({
	// The relative X position of the broadcast, from -1 to +1.
	x: z.optional(z.number()),

	// The relative Y position of the broadcast, from -1 to +1.
	y: z.optional(z.number()),

	// The relative zoom level of the broadcast, where 1 is 100%
	zoom: z.optional(z.number()),
})

export const LocationSchema = z.object({
	// The initial position of the broadcaster, from -1 to +1 in both dimensions.
	// This should be used for audio panning if supported.
	// If not provided, then the broadcaster is assumed to be at (0,0)
	initial: z.optional(PositionSchema),

	// If provided, then updates to the position are done via a separate Moq track.
	// This avoids a full catalog update every time we want to update 8* bytes.
	updates: z.optional(TrackSchema),

	// If provided, this broadcaster is requesting that other peers update their position.
	// The contents of the track are the same as the positionUpdates track above, just float32 pairs.
	peers: z.optional(z.record(z.uint32(), TrackSchema)),

	// If provided, the broadcaster allows other peers to request a position updates.
	// This is the key to the request record above, advertised in other catalogs.
	handle: z.optional(z.uint32()),
})

export type Location = z.infer<typeof LocationSchema>
export type Position = z.infer<typeof PositionSchema>