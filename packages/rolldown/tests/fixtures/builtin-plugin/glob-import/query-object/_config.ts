import { globImportPlugin } from "rolldown/experimental";
import { RolldownOutput } from "rolldown";
import { defineTest } from "@tests";
import { expect } from "vitest";
import * as fs from "node:fs";
import * as path from "path";

export default defineTest({
	config: {
		plugins: [
			globImportPlugin({}),
			{
				name: "load-file-with-query",
				load(id: string) {
					const [p, _] = id.split("?");
					const res = fs.readFileSync(p, "utf-8");
					return res;
				},
			},
		],
	},
	async afterTest(output: RolldownOutput) {
		output.output.forEach((chunk) => {
			if (chunk.type === "chunk") {
				switch (chunk.name) {
					case "b": {
						expect(chunk.code).toMatchFileSnapshot(path.resolve(import.meta.dirname, "dir/b.js.snap"));
					}
					case "dir_index": {
						expect(chunk.code).toMatchFileSnapshot(path.resolve(import.meta.dirname, "dir/index.js.snap"));
					}
				}
			} else {
				console.log("fuck");
			}
		});
		await import("./assert.mjs");
	},
});
