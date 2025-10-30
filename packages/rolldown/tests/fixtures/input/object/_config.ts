import { defineTest } from 'rolldown-tests';
import { getOutputChunkNames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: {
      main: 'main.js',
      entry: 'entry.js',
    },
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['entry.js', 'main.js']);
  },
});
