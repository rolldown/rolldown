import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-isCommonJS',
        moduleParsed: function(moduleInfo) {
          if (moduleInfo.id.endsWith('.cjs')) {
            // Check top-level property
            expect(moduleInfo.isCommonJS).toBe(true);
            // Check meta.commonjs.isCommonJS for backward compatibility with Rollup
            expect(moduleInfo.meta.commonjs.isCommonJS).toBe(true);
          } else if (moduleInfo.id.endsWith('.mjs')) {
            // Check top-level property
            expect(moduleInfo.isCommonJS).toBe(false);
            // Check meta.commonjs.isCommonJS for backward compatibility with Rollup
            expect(moduleInfo.meta.commonjs.isCommonJS).toBe(false);
          } else if (moduleInfo.id.endsWith('main.js')) {
            // main.js is ESM by default
            expect(moduleInfo.isCommonJS).toBe(false);
            expect(moduleInfo.meta.commonjs.isCommonJS).toBe(false);
          }
        },
      },
    ],
  },
});
