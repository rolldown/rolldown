import { defineTest } from 'rolldown-tests';
import { getOutputSourcemapFilenames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: [
      {
        dir: 'dist-a',
        sourcemap: true,
        sourcemapFileNames: '[name]-[hash].map',
        sourcemapPathTransform: (source) => `a/${source}`,
      },
      {
        dir: 'dist-b',
        sourcemap: true,
        sourcemapFileNames: '[name]-[hash].map',
        sourcemapPathTransform: (source) => `b/${source}`,
      },
    ],
  },
  afterTest: ([outputA, outputB]) => {
    expect(getOutputSourcemapFilenames(outputA)).not.toStrictEqual(
      getOutputSourcemapFilenames(outputB),
    );
  },
});
