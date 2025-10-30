import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      jsx: {
        refresh: true,
      },
    },
    external: ['react', 'react/jsx-runtime'],
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code.includes('$RefreshReg$')).toBe(true);
  },
});
