import { z } from "zod";

import { TrackSchema } from "./track";

export const VideoSchema = z.object({
	track: TrackSchema,
	codec: z.string(),
	description: z.string().optional(), // hex encoded
	resolution: z.object({
		width: z.number(),
		height: z.number(),
	}),
	framerate: z.number().optional(),
	bitrate: z.number().optional(),
});

export type Video = z.infer<typeof VideoSchema>;
