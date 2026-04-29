import { defineTest } from 'rolldown-tests';
import { getOutputSourcemapFilenames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      sourcemapFileNames: '[name]-abcde.js.map',
    },
  },
  afterTest: (output) => {
    expect(getOutputSourcemapFilenames(output)).toStrictEqual([
      'chunk-abcde.js.map',
      'main-abcde.js.map',
    ]);
  },
});
