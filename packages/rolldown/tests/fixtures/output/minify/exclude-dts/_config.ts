import { defineTest } from "rolldown-tests";
import { expect } from "vitest";
import * as path from "node:path";

export default defineTest({
  config: {
    output: {
      minify: true,
    },
    plugins: [
      {
        name: "test-plugin",
        buildStart() {
          this.emitFile({
            type: "chunk",
            id: "main.js",
            fileName: "main.d.ts",
          });
        },
      },
    ],
  },
  afterTest: (output) => {
    for (const o of output.output) {
      if (o.type !== "chunk") {
        expect(o.source).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'snap', `${o.fileName}.snap`),
        );
      } else {
        expect(o.code).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'snap',`${o.fileName}.snap`),
        );

      }
    }
  },
});
