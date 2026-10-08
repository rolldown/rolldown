import { defineTest } from 'rolldown-tests';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'insert_dummy_flag',
        transform(code) {
          return { code: `const __VITE_PRELOAD__ = [];` + code };
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
    external: ['node:assert'],
  },
  async afterTest(output) {
    // Importing the built chunk is the real assertion: a sync thunk would make
    // it fail to parse before any of this runs.
    await import('./assert.mjs');
    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        expect(item.code).not.toMatch(/__vitePreload\(\(\)\s*=>/);
        expect(item.code).toMatch(/__vitePreload\(async\s*\(\)\s*=>/);
      }
    });
  },
});
