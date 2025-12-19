import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      codeSplitting: 'none',
    },
  },
  afterTest: (output) => {
    // When codeSplitting is 'none', dynamic imports should be inlined
    // and there should be only one output chunk
    expect(output.output.length).toEqual(1);
    expect(output.output[0].code).toContain('"a"');
  },
});
