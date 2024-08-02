import { defineConfig } from "rolldown";
import { globImportPlugin } from "rolldown/experimental";
import * as fs from "fs";

export default defineConfig({
	input: "./index.js",
	resolve: {
		// This needs to be explicitly set for now because oxc resolver doesn't
		// assume default exports conditions. Rolldown will ship with a default that
		// aligns with Vite in the future.
		// conditionNames: ["import"],
	},
	plugins: [
		globImportPlugin({
      restoreQueryExtension: true,
    }),
		{
			name: "test",
			load(id) {
				const [p, _] = id.split("?");
				console.log(`p: `, p);
				const res = fs.readFileSync(p, "utf-8");
				return res;
			},
		},
	],
});
