import { defineTest } from "rolldown-tests";

export default defineTest({
	config: {
		experimental: {
			viteMode: true,
		},
		plugins: [
			{
				name: "custom-json",
				load(id) {
					if (id.endsWith(".json")) {
						return {
							code:
								'export const foo = "bar";' + 'export default {200: "ok", foo}',
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
