import { defineConfig } from "rolldown";

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
			name: "test-plugin",
			async closeBundle() {
        await new Promise((resolve) => {
          setTimeout(() => {
            console.log("test-plugin closeBundle");
            resolve();
          }, 1000);
        });
      },
		},
	],
	output: {
		plugins: [
			{
				name: "test-plugin",
				outputOptions: function (options) {
					options.banner = "/* banner */";
					return options;
				},
			},
		],
	},
	experimental: {
		enableComposingJsPlugins: true,
	},
});
