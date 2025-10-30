import { defineTest } from 'rolldown-tests';
import { getOutputChunkNames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.js',
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['main.js']);
  },
});
