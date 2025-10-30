import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.jsx',
    external: ['react'],
    transform: {
      jsx: 'react',
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code.includes('React.createElement')).toBe(true);
  },
});
