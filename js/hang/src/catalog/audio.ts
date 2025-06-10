import { z } from "zod/v4-mini";

import { TrackSchema } from "./track";

// Mirrors AudioDecoderConfig
// https://w3c.github.io/webcodecs/#audio-decoder-config
export const AudioConfigSchema = z.object({
	// See: https://w3c.github.io/webcodecs/codec_registry.html
	codec: z.string(),

	// The description is used for some codecs.
	// If provided, we can initialize the decoder based on the catalog alone.
	// Otherwise, the initialization information is in-band.
	description: z.optional(z.string()), // hex encoded TODO use base64

	// The sample rate of the audio in Hz
	sampleRate: z.uint32(),

	// The number of channels in the audio
	numberOfChannels: z.uint32(),

	// The bitrate of the audio in bits per second
	// TODO: Support up to Number.MAX_SAFE_INTEGER
	bitrate: z.optional(z.uint32()),
});

export const AudioSchema = z.object({
	// The MoQ track information.
	track: TrackSchema,

	// The configuration of the audio track
	config: AudioConfigSchema,
});

export type Audio = z.infer<typeof AudioSchema>;
export type AudioConfig = z.infer<typeof AudioConfigSchema>;
