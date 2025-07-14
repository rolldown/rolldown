import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

export default defineTest({
  config: {
    input: {
      main: "./entry.js",
    },
    experimental: {
      hmr: true,
    },
    treeshake: true,
  },
  afterTest: (output) => {
    expect(output.output[0].code).toContain(`return "foo";`);
    expect(output.output[0].code).not.toContain(`return "unused";`);
  },
});