import { basename } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['index.ts', 'index-1.ts'],
    plugins: [
      {
        name: 'keep-components',
        transform(code, id) {
          if (basename(id) === 'components.js') {
            return { code, moduleSideEffects: 'no-treeshake' };
          }
        },
      },
    ],
    output: {
      codeSplitting: {
        groups: ['a', 'b', 'c'].map((name) => ({ name: `bundle-${name}`, test: `${name}.js` })),
      },
    },
  },
  afterTest: (output) => {
    expect(getOutputChunk(output).map((chunk) => chunk.name)).toContain('components');
  },
});
