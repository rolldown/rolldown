import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'cjs',
    },
  },
  afterTest: (output) => {
    // preset defaults to `'es2015'` which should set symbols: true, which includes Symbol.toStringTag
    expect(output.output[0].code).toContain('Symbol.toStringTag');
  },
});
