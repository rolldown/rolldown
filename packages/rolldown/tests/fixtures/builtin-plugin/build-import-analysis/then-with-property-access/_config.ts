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
        // Should transform await import().then() pattern
        expect(item.code).to.include('__vitePreload');
      }
      if (item.type === 'chunk' && item.name === 'lib') {
        // Verify tree-shaking: unused export should not be in the lib chunk
        expect(item.code).to.not.include('unused');
        // The used exports should still be present
        expect(item.code).to.include('foo');
        expect(item.code).to.include('bar');
      }
    });
  },
});
