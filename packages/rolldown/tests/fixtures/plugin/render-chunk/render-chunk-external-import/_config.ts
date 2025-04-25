import { expect } from "vitest";
import { defineTest } from "rolldown-tests";

export default defineTest({
	config: {
		plugins: [
			{
				name: "test-plugin",
				renderChunk(_code, chunk) {
					expect(chunk.imports).toEqual(["node:http"]);
				},
				generateBundle(_opts, bundle) {
					for (const chunk of Object.values(bundle)) {
						if (chunk.type !== "chunk") continue;
						expect(chunk.imports).toEqual(["node:http"]);
					}
				},
			},
		],
	},
});
