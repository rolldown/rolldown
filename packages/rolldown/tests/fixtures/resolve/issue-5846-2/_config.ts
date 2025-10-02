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
				async resolveDynamicImport(source, importer) {
					if (!importer) {
						return;
					}
					const resolved = await this.resolve(source, importer, {
						skipSelf: true,
					});

					// Comment out this line to "fix" tree-shaking.
					return resolved;
				},
			},
		],
	},
	afterTest(output) {
		for (const chunk of output.output) {
			if (chunk.type !== "chunk") continue;
			expect(chunk.code).not.toContain(`sideEffects`);
		}
	},
});
