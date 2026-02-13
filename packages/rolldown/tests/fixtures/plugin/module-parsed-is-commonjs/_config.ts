import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test',
        moduleParsed: function(moduleInfo) {
          if (moduleInfo.id.endsWith('.cjs')) {
            // Check top-level property
            expect(moduleInfo.isCommonJS).toBe(true);
          } else if (moduleInfo.id.endsWith('.mjs')) {
            // Check top-level property
            expect(moduleInfo.isCommonJS).toBe(false);
          } else if (moduleInfo.id.endsWith('main.js')) {
            // main.js is ESM by default
            expect(moduleInfo.isCommonJS).toBe(false);
          }
        },
      },
    ],
  },
});