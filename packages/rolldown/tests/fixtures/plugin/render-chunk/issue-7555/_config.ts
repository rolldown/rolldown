import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    experimental: {
      attachDebugInfo: 'none',
    },
    plugins: [
      {
        name: 'assert-no-empty-import-prelude',
        renderChunk(code) {
          expect(code.startsWith('function throwError')).toBe(true);
        },
      },
    ],
    output: {
      format: 'cjs',
    },
  },
});
