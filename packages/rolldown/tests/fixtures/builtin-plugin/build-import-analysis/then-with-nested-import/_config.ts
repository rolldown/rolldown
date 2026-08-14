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
        // Should transform both the outer and inner import() calls
        expect(item.code).to.include('__vitePreload');
        // Count occurrences of __vitePreload - should have at least 2 (one for each import)
        const preloadCount = (item.code.match(/__vitePreload\(/g) || []).length;
        expect(preloadCount).to.be.greaterThanOrEqual(2);
      }
      if (item.type === 'chunk' && item.name === 'lib1') {
        // foo should be present since it's accessed in the then callback
        expect(item.code).to.include('foo');
        // Verify tree-shaking: unused1 should not be in the lib1 chunk
        expect(item.code).to.not.include('unused1');
      }
      if (item.type === 'chunk' && item.name === 'lib2') {
        // bar should be present since it's accessed in the second then callback
        expect(item.code).to.include('bar');
        // Verify tree-shaking: unused2 should not be in the lib2 chunk
        expect(item.code).to.not.include('unused2');
      }
    });
  },
});
