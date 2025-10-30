import { defineTest } from 'rolldown-tests';
import { getOutputChunkNames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: {
      a: './main.js',
      b: './main.js',
    },
  },
  afterTest: function(output) {
    let chunkNames = getOutputChunkNames(output).sort();
    // two entry chunks + shared chunk
    expect(chunkNames.length).toStrictEqual(3);
    expect(chunkNames).toContain('a.js');
    expect(chunkNames).toContain('b.js');
  },
});
