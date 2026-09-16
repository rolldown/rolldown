import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'remove-chunk',
        generateBundle(outputOptions, bundle) {
          delete bundle['main.js'];
          expect(bundle['main.js']).toBeUndefined();
          expect('main.js' in bundle).toBe(false);
          expect(Object.keys(bundle)).not.toContain('main.js');
        },
      },
    ],
  },
});
