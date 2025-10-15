import { expect } from "vitest";
import { defineTest } from "rolldown-tests";

export default defineTest({
	config: {
		input: ["main.js"],
		experimental: {
			nativeMagicString: true,
		},
		output: {
			sourcemap: true,
		},
		plugins: [
			{
				name: "test",
				transform(code, id, meta) {
					let { magicString } = meta;
					if (magicString) {
						magicString.replace("test", "result");
						return {
							code: magicString,
						};
					}
				},
			},
		],
	},
	afterTest: function (output) {
		expect(output.output[0].map).toBeDefined();
		expect(output.output[0].map!.toString()).toMatchInlineSnapshot(
			`"{"version":3,"file":"main.js","names":[],"sources":["../main.js"],"sourcesContent":["export const test = 'res';\\n"],"mappings":";AAAA,MAAa,SAAI"}"`,
		);
	},
});
