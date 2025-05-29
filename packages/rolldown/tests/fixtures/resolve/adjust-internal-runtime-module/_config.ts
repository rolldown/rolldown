import { defineTest } from "rolldown-tests";
import path from "node:path";
import { expect } from "vitest";
const entry = path.join(__dirname, "./main.ts");

export default defineTest({
	config: {
		input: entry,
		platform: "node",
		plugins: [
			{
				name: "test",
				resolveId: {
					filter: {
						id: /^node:module$/,
					},
					handler(id, importer) {
						if (importer == "rolldown:runtime") {
							return {
								moduleSideEffects: false,
								id: "module",
								external: true,
							};
						}
					},
				},
			},
		],
	},
	afterTest(output) {
		expect(output.output[0].code).toContain(`"module"`);
		expect(output.output[0].code).toContain(`"node:module"`);
	},
});
