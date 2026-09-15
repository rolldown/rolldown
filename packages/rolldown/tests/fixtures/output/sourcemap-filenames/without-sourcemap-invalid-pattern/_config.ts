import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      sourcemap: false,
      sourcemapFileNames: () => '/absolute.map',
    },
  },
  afterTest: () => {
    expect.unreachable('invalid sourcemapFileNames pattern should fail');
  },
  catchError: (error: unknown) => {
    expect(String(error)).toContain('patterns can be neither absolute nor relative');
  },
});
