import { defineTest } from "rolldown-tests";
import { expect } from "vitest";
import * as path from "node:path";

export default defineTest({
  config: {
    transform: {
      target: "chrome88"
    },
    output: {
      minify: true,
    },
  },
  afterTest: async (output) => {
    for (const o of output.output) {
      if (o.type !== "chunk") {
        await expect(o.source).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'snap', `${o.fileName}.snap`),
        );
      } else {
        await expect(o.code).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'snap',`${o.fileName}.snap`),
        );
      }
    }
  },
});
