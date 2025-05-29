import { z } from "zod/v4-mini"

import { CapabilitiesSchema } from "./capabilities"
import { LocationsSchema } from "./location"

export const FeedbackSchema = z.object({
	capabilities: z.optional(CapabilitiesSchema),
	locations: z.optional(LocationsSchema),
})

export type Feedback = z.infer<typeof FeedbackSchema>