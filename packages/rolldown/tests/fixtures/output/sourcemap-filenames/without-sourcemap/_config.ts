import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let calls = 0;

export default defineTest({
  sequential: true,
  config: {
    output: {
      sourcemap: false,
      sourcemapFileNames: () => {
        calls += 1;
        return '[name].map';
      },
    },
  },
  afterTest: () => {
    expect(calls).toBe(1);
  },
});
