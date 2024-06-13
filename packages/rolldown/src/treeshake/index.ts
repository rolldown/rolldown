import { z } from "zod";
import {
	ModuleSideEffectsOptionSchema,
	NormalizedTreeshakingOptionsSchema,
} from "./module-side-effects";

export const TreeshakingOptionsSchema =
	NormalizedTreeshakingOptionsSchema.extend({
		moduleSideEffects: ModuleSideEffectsOptionSchema.optional(),
	});

export type TreeshakingOptions = z.infer<typeof TreeshakingOptionsSchema>;
export * from './module-side-effects'
