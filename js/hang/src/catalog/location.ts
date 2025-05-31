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
	// If not provided, then the broadcaster is assumed to be at (0,0)
	// This should be used for audio panning but can also be used for video positioning.
	initial: z.optional(PositionSchema),

	// If provided, then updates to the position are done via a separate Moq track.
	// This is used to avoid a full catalog update every time we want to update a few bytes.
	// TODO: These updates currently use JSON for simplicity, but we should use a binary format.
	updates: z.optional(TrackSchema),

	// If true, then this broadcaster allows other peers to request position updates.
	// We will have to discover and subscribe to their position updates.
	peering: z.optional(z.boolean()),

	// If provided, this broadcaster is signaling the location of other peers.
	// The key is the name of the broadcast, and the value is the track that contains the position updates.
	peers: z.optional(z.record(z.string(), TrackSchema)),
})

export type Location = z.infer<typeof LocationSchema>
export type Position = z.infer<typeof PositionSchema>