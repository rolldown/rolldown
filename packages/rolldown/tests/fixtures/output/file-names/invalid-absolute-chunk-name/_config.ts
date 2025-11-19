import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      manualChunks(id) {
        if (id.includes('foo.js')) {
          return '/absolute/path';
        }
        return null;
      },
    },
  },
  catchError(error: any) {
    expect(error.message).toContain(
      'Invalid substitution "/absolute/path" for placeholder "[name]"',
    );
    expect(error.message).toContain(
      'can be neither absolute nor relative path',
    );
  },
});
