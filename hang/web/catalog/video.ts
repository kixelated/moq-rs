import { z } from "zod";

import { TrackSchema } from "./track";

export const VideoSchema = z.object({
	track: TrackSchema,
	codec: z.string(),
	resolution: z.object({
		width: z.number(),
		height: z.number(),
	}),
});

export type Video = z.infer<typeof VideoSchema>;
