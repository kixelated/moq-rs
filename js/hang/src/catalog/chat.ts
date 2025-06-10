import { z } from "zod/v4-mini";
import { TrackSchema } from "./track";

export const ChatSchema = z.object({
	track: TrackSchema,

	// The number of milliseconds since the Unix epoch, representing timestamp zero.
	// Each chat message has a timestamp relative to this epoch.
	epoch: z.optional(z.number()),
});

export type Chat = z.infer<typeof ChatSchema>;
