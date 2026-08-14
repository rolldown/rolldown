import { defineTest } from 'rolldown-tests';
import { getOutputChunkNames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js', 'entry.js'],
  },
  afterTest: function (output) {
    expect(getOutputChunkNames(output)).toStrictEqual(['entry.js', 'main.js']);
  },
});
