import { z } from "zod/v4-mini";
import { TrackSchema } from "./track";

export const ChatSchema = z.object({
	track: TrackSchema,

	// If provided, the number of milliseconds before messages should be deleted.
	ttl: z.optional(z.number()),
});

export type Chat = z.infer<typeof ChatSchema>;
