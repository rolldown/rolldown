import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    output: {
      inlineDynamicImports: true,
      manualChunks: (id) => {
        if (id.includes('lib')) {
          return 'vendor';
        }
      },
    },
  },
  catchError(error: any) {
    expect(error.message).toContain(
      'Invalid value "true" for option "output.inlineDynamicImports" - this option is not supported for "output.manualChunks".',
    );
  },
});
