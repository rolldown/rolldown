import { defineTest } from "rolldown-tests";

export default defineTest({
	config: {
    experimental: {
      viteMode: true
    },
		plugins: [
			{
				name: "custom-json",
				load(id) {
					if (id.endsWith(".json")) {
						return {
							code: 'export default {200: "ok"}',
							moduleType: "js",
						};
					}
				},
			},
		],
	},
	afterTest: async () => {
		await import("./_test.mjs");
	},
});
