import { defineConfig } from "rolldown";

export default defineConfig({
	input: "./entry.js",
	resolve: {
		// This needs to be explicitly set for now because oxc resolver doesn't
		// assume default exports conditions. Rolldown will ship with a default that
		// aligns with Vite in the future.
		conditionNames: ["import"],
	},

	plugins: [
		{
			name: "ignore-side-effects",
			transform(_code, id) {
				return { moduleSideEffects: false };
			},
		},
	],
	experimental: {
		// enableComposingJsPlugins: true,
	},
});
