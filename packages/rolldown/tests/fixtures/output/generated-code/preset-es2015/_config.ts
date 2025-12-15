import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'cjs',
      generatedCode: {
        preset: 'es2015',
      },
    },
  },
  afterTest: (output) => {
    // preset: 'es2015' should set symbols: true, which includes Symbol.toStringTag
    expect(output.output[0].code).toContain('Symbol.toStringTag');
  },
});
