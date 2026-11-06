import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      entryFileNames: 'entry.js',
      codeSplitting: false,
    },
  },
  afterTest: (output) => {
    const entry = output.output.find(
      (item) => item.type === 'chunk' && item.fileName === 'entry.js',
    );
    if (entry?.type !== 'chunk') {
      throw new Error('entry.js should be emitted as a chunk');
    }
    expect(entry.dynamicImports).toStrictEqual([]);
  },
});
