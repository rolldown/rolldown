import type { OutputChunk as RolldownOutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputAsset, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'mutate-inline-sourcemap-chunk',
        generateBundle(_options, bundle) {
          const chunk = bundle['main.js'] as RolldownOutputChunk;
          chunk.code = `${chunk.code}\nconsole.error('updated');`;
          const map = chunk.map;
          expect(map).not.toBeNull();
          chunk.map = map;
        },
      },
    ],
    output: {
      sourcemap: 'inline',
    },
  },
  afterTest: (output) => {
    expect(getOutputChunk(output)).toHaveLength(1);
    expect(getOutputAsset(output)).toHaveLength(0);
  },
});
