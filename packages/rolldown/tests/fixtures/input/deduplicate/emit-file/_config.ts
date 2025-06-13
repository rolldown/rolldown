import { defineTest } from "rolldown-tests";
import { expect } from "vitest";
import { getOutputChunkNames } from "rolldown-tests/utils";

export default defineTest({
	config: {
    input: ["./main.js"],
    plugins: [
      {
        name: "test",
        buildStart() {
          this.emitFile({
            type: "chunk",
            id: "main.js",
          });
        },
      },
    ]
	},
	afterTest: function (output) {
		let chunkNames = getOutputChunkNames(output).sort();
    // two entry chunks + shared chunk
		expect(chunkNames.length).toStrictEqual(3);
	},
});
