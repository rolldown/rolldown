import { defineConfig } from "rolldown";
import { jsonPlugin } from "rolldown/experimental";

export default defineConfig({
	input: "./index.js",
	resolve: {
		// This needs to be explicitly set for now because oxc resolver doesn't
		// assume default exports conditions. Rolldown will ship with a default that
		// aligns with Vite in the future.
		conditionNames: ["import"],
	},

	plugins: [jsonPlugin({ stringify: true, isBuild: true })],
});
