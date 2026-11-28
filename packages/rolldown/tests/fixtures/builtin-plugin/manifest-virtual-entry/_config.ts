import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteManifestPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

// The root is the fixture directory, while the process cwd is the test runner's.
export default defineTest({
  sequential: true,
  config: {
    input: 'virtual:entry',
    plugins: [
      viteManifestPlugin({
        root: path.resolve(import.meta.dirname),
        outPath: 'manifest.json',
        cssEntries: () => ({}),
      }),
      {
        name: 'virtual-entry',
        resolveId(id) {
          if (id === 'virtual:entry') return '\0virtual:entry';
        },
        load(id) {
          if (id === '\0virtual:entry') return 'export default 1';
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const manifest = await import('./dist/manifest.json');
    expect(Object.keys(manifest.default)).toEqual(['virtual:entry']);
  },
});
