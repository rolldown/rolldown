import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// Same as `legacy-decorators-js-esnext`, with no target and without tsconfig
// discovery, so the options are resolved once rather than per file.
export default defineTest({
  config: {
    input: 'main.js',
    tsconfig: false,
    transform: {
      decorator: { legacy: true },
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code).not.toContain('@dec');
    expect(chunk.code).toMatch(/__decorate\(\[dec\], C\.prototype, "m", null\)/);
  },
});
