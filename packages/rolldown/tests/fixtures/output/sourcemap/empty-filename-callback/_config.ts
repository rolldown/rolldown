import { defineTest } from 'rolldown-tests';
import { getOutputFileNames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
      sourcemapFileNames: () => '',
    },
  },
  afterTest(output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js', 'main.js.map']);
    expect(output.output[0].code).toContain('//# sourceMappingURL=main.js.map');
  },
});
