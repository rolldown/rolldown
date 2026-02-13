import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        // insert some dummy runtime flag to assert the runtime behavior
        name: 'insert_dummy_flag',
        transform(code) {
          let runtimeCode = `const __VITE_PRELOAD__ = [];`;
          return {
            code: runtimeCode + code,
          };
        },
      },
      viteBuildImportAnalysisPlugin({
        preloadCode: `export const __vitePreload = (v) => { return v() };`,
        insertPreload: true,
        optimizeModulePreloadRelativePaths: false,
        renderBuiltUrl: false,
        isRelativeBase: false,
      }),
    ],
  },
  async afterTest(output) {
    await import('./assert.mjs');
    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        // Check that the .then() callback is preserved
        expect(item.code).to.includes('.then((m) => m.foo)');
        expect(item.code).to.includes('.then((m) => m.bar)');
        expect(item.code).to.includes('.then((m) => m.nested.value)');
      }
    });
    
    // Check tree-shaking: unused exports should not be in the main chunk
    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        const code = (item as OutputChunk).code;
        // The unused export should not be used in the main chunk
        // It may still be exported from lib.js, but shouldn't be accessed
        expect(code.match(/\.unused\b/g)).toBeNull();
        expect(code.match(/\bunused:/g)).toBeNull();
      }
    });
  },
});
