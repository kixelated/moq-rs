import { z } from "zod/v4-mini";

import { TrackSchema } from "./track";

export const DimensionsSchema = z.object({
	width: z.uint32(),
	height: z.uint32(),
});

export type Dimensions = z.infer<typeof DimensionsSchema>;

// Mirrors VideoDecoderConfig
// https://w3c.github.io/webcodecs/#video-decoder-config
export const VideoSchema = z.object({
	// The MoQ track information.
	track: TrackSchema,

	// See: https://w3c.github.io/webcodecs/codec_registry.html
	codec: z.string(),

	// The description is used for some codecs.
	// If provided, we can initialize the decoder based on the catalog alone.
	// Otherwise, the initialization information is (repeated) before each key-frame.
	description: z.optional(z.string()), // hex encoded

	// The width and height of the video in pixels
	dimensions: z.optional(DimensionsSchema),

	// Ratio of display width/height to coded width/height
	// Allows stretching/squishing individual "pixels" of the video
	// If not provided, the display ratio is 1:1
	displayRatio: z.optional(DimensionsSchema),

	// The frame rate of the video in frames per second
	framerate: z.optional(z.uint32()),

	// The bitrate of the video in bits per second
	// TODO: Support up to Number.MAX_SAFE_INTEGER
	bitrate: z.optional(z.uint32()),

	// If true, the decoder will optimize for latency.
	// Default: true
	optimizeForLatency: z.optional(z.boolean()),

	// The rotation of the video in degrees.
	// Default: 0
	rotation: z.optional(z.number()),

	// If true, the decoder will flip the video horizontally
	// Default: false
	flip: z.optional(z.boolean()),
});

export type Video = z.infer<typeof VideoSchema>;
