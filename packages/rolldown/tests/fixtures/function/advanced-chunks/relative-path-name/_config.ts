import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './entry.js',
    output: {
      codeSplitting: {
        groups: [
          {
            name: (file) => {
              if (file.includes('src')) {
                return 'base/src.js';
              }
            },
          },
        ],
      },
      chunkFileNames: '[name].js',
    },
  },
  afterTest(output) {
    function findChunkStartWith(prefix: string) {
      const finded = output.output.find(
        (chunk) => chunk.type === 'chunk' && chunk.fileName.startsWith(prefix),
      );
      if (!finded) {
        throw new Error(`chunk ${prefix} not found`);
      }
      if (finded.type !== 'chunk') {
        throw new Error('should be chunk');
      }
      return finded;
    }
    const entry = findChunkStartWith('entry');
    const base = findChunkStartWith('base/src.js');

    expect(entry.moduleIds).toMatchObject([/entry.js$/]);

    expect(base.moduleIds).toMatchObject([/a.js$/, /b.js$/]);
  },
});
