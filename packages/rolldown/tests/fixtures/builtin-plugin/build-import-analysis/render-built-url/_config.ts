import { defineTest } from 'rolldown-tests';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'replace_preload_markers',
        renderChunk(code) {
          return code.replace(/__VITE_PRELOAD__[\da-f]{32}/g, '[]');
        },
      },
      viteBuildImportAnalysisPlugin({
        preloadCode: `export const __vitePreload = (v) => { return v() };`,
        insertPreload: true,
        optimizeModulePreloadRelativePaths: false,
        renderBuiltUrl: true,
        isRelativeBase: false,
      }),
    ],
  },
  async afterTest(output) {
    await import('./assert.mjs');
    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        expect(item.code).to.includes('import.meta.url');
      }
    });
  },
});
