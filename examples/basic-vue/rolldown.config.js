import { defineConfig } from "rolldown";
import {buildImportAnalysisPlugin} from 'rolldown/experimental'

export default defineConfig({
	input: "./index.js",
	resolve: {
		// This needs to be explicitly set for now because oxc resolver doesn't
		// assume default exports conditions. Rolldown will ship with a default that
		// aligns with Vite in the future.
		conditionNames: ["import"],
	},

	plugins: [
		{
			// insert some dummy runtime flag to assert the runtime behavior
			name: "insert_dummy_flag",
			transform(code, id) {
				let runtimeCode = `
const __VITE_IS_MODERN__ = false;

`;
				return {
					code: runtimeCode + code,
				};
			},
		},
		buildImportAnalysisPlugin({
			preloadCode: `
export const __vitePreload = (v) => {
  return v
};
`,
			insertPreload: true,
			optimizeModulePreloadRelativePaths: false,
		}),
	],
});
