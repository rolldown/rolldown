import { defineTest } from "rolldown-tests";
import { expect, vi} from "vitest";
import { getOutputChunkNames } from "rolldown-tests/utils";

let names: string[] = []
let fn = vi.fn(() => {})
export default defineTest({
	config: {
		input: ["./main.js"],
		plugins: [
			{
				name: "test",
				buildStart() {
					let a = this.emitFile({
						type: "chunk",
						id: "./main.js",
						fileName: "main.d.ts",
					});
					let b = this.emitFile({
						type: "chunk",
						id: "./main.js",
						fileName: "main.d.ts",
					});
          names.push(a, b);
				},
				generateBundle(_) {
          expect(this.getFileName(names[0])).toStrictEqual("main.d.ts");
          expect(this.getFileName(names[1])).toStrictEqual("main.d.ts");
				},
        renderChunk() {
          fn();
        }
			},
		],
	},
	afterTest: function () {
    // entry, main.d.ts and a shared chunk
    expect(fn).toHaveBeenCalledTimes(3);
	},
});
