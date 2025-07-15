import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

export default defineTest({
  config: {
    output: {
      minify: 'dce-only',
    },
  },
  afterTest: (output) => {
    const code = output.output[0].code;
    expect(code).toContain('legal comment is kept');
    expect(code).toContain('annotation comment is kept');
  },
});
