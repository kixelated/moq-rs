import { z } from "zod";

export const LocationSchema = z.object({
	x: z.number(),
	y: z.number(),
});

export type Location = z.infer<typeof LocationSchema>;
