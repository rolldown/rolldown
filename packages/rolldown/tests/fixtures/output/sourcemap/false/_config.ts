import { defineTest } from 'rolldown-tests';
import { getOutputFileNames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: false,
    },
  },
  afterTest: function(output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js']);
  },
});
