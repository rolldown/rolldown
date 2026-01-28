import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteManifestPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    output: {
      chunkFileNames: '[name].js',
      assetFileNames: '[name][extname]',
    },
    plugins: [
      viteManifestPlugin({
        root: path.resolve(import.meta.dirname),
        outPath: 'manifest.json',
        cssEntries: () => Object.fromEntries(new Map().entries()),
      }),
      {
        name: 'test',
        buildStart() {
          this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: 'hello world',
            originalFileName: 'asset.txt',
          });
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const manifest = await import('./dist/manifest.json');
    await expect(manifest.default).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'manifest.json.snap'),
    );
  },
});
