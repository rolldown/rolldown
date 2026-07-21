import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let renderChunkCalls = 0;

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
          renderChunkCalls++;
          expect(code).toMatch(/^function throwError/);
        },
      },
    ],
    output: {
      format: 'cjs',
    },
  },
  afterTest: () => {
    expect(renderChunkCalls).toBe(1);
  },
});
