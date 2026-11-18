import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    external: () => true,
  },
  afterTest(output) {
    const code = getOutputChunk(output)[0].code;
    expect(code).toContain(`import { a } from "a" with { type: "it's" };`);
    expect(code).toContain('import { b } from "b" with { type: "back\\\\slash" };');
    expect(code).toContain('import { d } from "d" with { type: "new\\nline" };');
    expect(code).toContain(`import("c", { with: { type: "quote'inside" } });`);
    expect(code).toContain('import("e", { with: { type: "x\\u2028y" } });');
    expect(code).toContain('import("f", { with: { type: "double\\"quote" } });');
  },
});
