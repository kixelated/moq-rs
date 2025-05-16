import { z } from "zod";

import { TrackSchema } from "./track";

export const AudioSchema = z.object({
	track: TrackSchema,
	codec: z.string(),
	sample_rate: z.number(),
	channel_count: z.number(),
	bitrate: z.number().optional(),
});

export type Audio = z.infer<typeof AudioSchema>;
