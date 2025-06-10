import { z } from "zod/v4-mini";

export const UserSchema = z.object({
	id: z.optional(z.string()),
	name: z.optional(z.string()),
	avatar: z.optional(z.url()), // TODO allow using a track?
	color: z.optional(z.string()),
});

export type User = z.infer<typeof UserSchema>;
