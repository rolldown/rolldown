import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// Legacy decorators lower at any target, including `esnext`, which otherwise
// leaves plain JS files untransformed.
export default defineTest({
  config: {
    input: 'main.js',
    transform: {
      target: 'esnext',
      decorator: { legacy: true },
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code).not.toContain('@dec');
    expect(chunk.code).toMatch(/__decorate\(\[dec\], C\.prototype, "m", null\)/);
  },
});
