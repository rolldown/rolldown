import { defineTest } from 'rolldown-tests';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    output: {
      minify: { mangle: false },
    },
    plugins: [
      viteBuildImportAnalysisPlugin({
        preloadCode: `export const __vitePreload = (v) => { return v() };`,
        insertPreload: true,
        optimizeModulePreloadRelativePaths: false,
        renderBuiltUrl: false,
        isRelativeBase: false,
      }),
    ],
  },
  afterTest(output) {
    const entry = output.output.find((item) => item.type === 'chunk' && item.name === 'main');
    expect(entry?.type).toBe('chunk');
    if (entry?.type !== 'chunk') return;

    const markers = entry.code.match(/__VITE_PRELOAD__[\da-f]{32}/g) ?? [];
    expect(markers).toHaveLength(3);
    expect(new Set(markers)).toHaveLength(3);
    expect(entry.code.match(/__vitePreload\(/g)).toHaveLength(3);
  },
});
