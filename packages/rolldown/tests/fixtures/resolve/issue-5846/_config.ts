import { defineTest } from "rolldown-tests";
import path from "node:path";
import { expect } from "vitest";
const entry = path.join(__dirname, "./main.ts");

export default defineTest({
	config: {
		input: entry,
		plugins: [
			{
				name: "test",
				async resolveId(source, importer, options) {
					if (!importer) {
						return;
					}
					const resolved = await this.resolve(source, importer, {
						...options,
						skipSelf: true,
					});

					// Comment out this line to "fix" tree-shaking.
					return resolved;
				},
			},
		],
	},
	afterTest(output) {
		expect(output.output[0].code).not.toContain(`sideEffects`);
	},
});
