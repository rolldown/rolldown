import { defineTest } from 'rolldown-tests';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';

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
        renderBuiltUrl: false,
        isRelativeBase: false,
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
