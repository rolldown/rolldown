import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// `decorator.lower: true` lowers standard decorators in plain JS files at
// `esnext`, which keeps them by default.
export default defineTest({
  config: {
    input: 'main.js',
    transform: {
      target: 'esnext',
      decorator: { lower: true },
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code).not.toContain('@dec');
    expect(chunk.code).toContain('decoratorStart');
  },
});
