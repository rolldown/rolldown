import { defineTest } from 'rolldown-tests';
import { getOutputSourcemapFilenames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const literalPlaceholder = '!~{00000000000000000}~';

export default defineTest({
  config: {
    output: {
      sourcemap: true,
      sourcemapFileNames: `[name]-${literalPlaceholder}-[hash].map`,
    },
  },
  afterTest: (output) => {
    expect(getOutputSourcemapFilenames(output)[0]).toContain(literalPlaceholder);
  },
});
