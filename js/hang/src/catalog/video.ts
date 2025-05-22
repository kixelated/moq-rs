import { z } from "zod/v4-mini";

import { TrackSchema } from "./track";

export const VideoConfigSchema = z.object({
	// See: https://w3c.github.io/webcodecs/codec_registry.html
	codec: z.string(),

	// The description is used for some codecs.
	// If provided, we can initialize the decoder based on the catalog alone.
	// Otherwise, the initialization information is (repeated) before each key-frame.
	description: z.optional(z.string()), // hex encoded

	// The width and height of the video in pixels
	codedWidth: z.optional(z.uint32()),
	codedHeight: z.optional(z.uint32()),

	// Ratio of display width/height to coded width/height
	// Allows stretching/squishing individual "pixels" of the video
	// If not provided, the display ratio is 1:1
	displayRatioWidth: z.optional(z.uint32()),
	displayRatioHeight: z.optional(z.uint32()),

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

// Mirrors VideoDecoderConfig
// https://w3c.github.io/webcodecs/#video-decoder-config
export const VideoSchema = z.object({
	// The MoQ track information.
	track: TrackSchema,

	// The configuration of the video track
	config: VideoConfigSchema,
});

export type Video = z.infer<typeof VideoSchema>;
export type VideoConfig = z.infer<typeof VideoConfigSchema>;
