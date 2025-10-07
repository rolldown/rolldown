import { defineTest } from "rolldown-tests";
import { expect } from "vitest";
import * as path from "node:path";

export default defineTest({
  config: {
    transform: {
      target: "chrome88"
    },
    output: [
      { minify: true },
      { minify: 'dce-only' },
      { minify: { compress: true } }
    ],
  },
  afterTest: async (outputs) => {
    for (const [index, output] of outputs.entries()) {
      for (const o of output.output) {
        if (o.type !== "chunk") {
          await expect(o.source).toMatchFileSnapshot(
            path.resolve(import.meta.dirname, `snap-${index}`, `${o.fileName}.snap`),
          );
        } else {
          await expect(o.code).toMatchFileSnapshot(
            path.resolve(import.meta.dirname, `snap-${index}`,`${o.fileName}.snap`),
          );
        }
      }
    }
  },
});
