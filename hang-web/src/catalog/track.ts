import { z } from "zod";

export type GroupOrder = "desc" | "asc";

export const TrackSchema = z.object({
	name: z.string(),
	priority: z.number(),
	bitrate: z.number().optional(),
});

export type Track = z.infer<typeof TrackSchema>;
