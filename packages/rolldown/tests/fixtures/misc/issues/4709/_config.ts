import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

export default defineTest({
	config: {
		input: ["./main.js"],
		output: {
			dir: "./dist",
		},
		plugins: [
			{
				name: "emit",
				resolveId(id) {
					if (id[0] === "\0") {
						return id;
					}
				},
				load(id) {
					if (id[0] === "\0") {
						return `console.log('virtual')`;
					}
				},
				renderChunk(_code, chunk) {
					if (chunk.name.includes("virtual")) {
						expect(JSON.stringify(chunk.name)).toBe("\"_virtual\"");
					}
				},
			},
		],
	},
});
