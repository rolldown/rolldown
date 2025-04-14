import { defineConfig, withFilter } from "rolldown";
function transformFn(name) {
	console.log(`${name} called`);
}

/**
 * @type import('rolldown').RolldownPluginOption
 * */
const nestedPlugin = {
	name: "test-plugin-1-1",
	resolveId(_, _id) {
		transformFn("test-plugin-1-1");
	},
};

export default defineConfig({
	input: "./index.js",
	resolve: {
		// This needs to be explicitly set for now because oxc resolver doesn't
		// assume default exports conditions. Rolldown will ship with a default that
		// aligns with Vite in the future.
		conditionNames: ["import"],
	},
	plugins: [
		withFilter(nestedPlugin, {
			resolveId: {
				id: /\.vue$/,
			},
		}),
	],
	output: {
		// plugins: [
		// 	{
		// 		name: "test-plugin",
		// 		outputOptions: function (options) {
		// 			options.banner = "/* banner */";
		// 			return options;
		// 		},
		// 	},
		// ],
	},
	experimental: {
		// enableComposingJsPlugins: true,
	},
});
