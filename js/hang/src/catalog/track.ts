import { z } from "zod";

export const TrackSchema = z.object({
	name: z.string(),
	priority: z.number(),
	bitrate: z.number().optional(),
});

export type Track = z.infer<typeof TrackSchema>;
