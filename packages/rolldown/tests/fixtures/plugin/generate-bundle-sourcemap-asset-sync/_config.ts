import type { OutputChunk as RolldownOutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputAsset, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js', 'automatic.js', 'deleted.js'],
    plugins: [
      {
        name: 'mutate-output-chunks',
        generateBundle(_options, bundle) {
          const main = bundle['main.js'] as RolldownOutputChunk;
          const automatic = bundle['automatic.js'] as RolldownOutputChunk;
          const deleted = bundle['deleted.js'] as RolldownOutputChunk;

          main.code = 'console.error("main")';
          const mainMap = main.map!;
          mainMap.file = 'renamed-main.js';
          mainMap.mappings = `;${mainMap.mappings}`;
          mainMap.sources.push('updated-main.js');
          main.map = mainMap;
          main.fileName = 'renamed-main.js';

          automatic.code = 'console.error("automatic")';
          const automaticMap = automatic.map!;
          automaticMap.mappings = `;${automaticMap.mappings}`;
          automaticMap.sources.push('updated-automatic.js');
          automatic.map = automaticMap;

          const deletedMap = deleted.map!;
          deletedMap.mappings = `;${deletedMap.mappings}`;
          deleted.map = deletedMap;
          delete bundle[deleted.sourcemapFileName!];

          const explicitAsset = bundle[main.sourcemapFileName!];
          expect(explicitAsset?.type).toBe('asset');
          if (explicitAsset?.type === 'asset') {
            explicitAsset.source = 'plugin-owned-sourcemap';
          }
        },
      },
    ],
    output: {
      chunkFileNames: '[name].js',
      sourcemap: true,
      sourcemapFileNames: 'maps/[name].map',
    },
  },
  afterTest: (output) => {
    const chunks = getOutputChunk(output);
    const assets = getOutputAsset(output);

    const main = chunks.find((chunk) => chunk.fileName === 'renamed-main.js');
    const automatic = chunks.find((chunk) => chunk.fileName === 'automatic.js');
    const deleted = chunks.find((chunk) => chunk.fileName === 'deleted.js');
    expect(main).toBeDefined();
    expect(automatic).toBeDefined();
    expect(deleted).toBeDefined();

    const mainAsset = assets.find((asset) => asset.fileName === main!.sourcemapFileName);
    expect(mainAsset?.source).toBe('plugin-owned-sourcemap');

    const automaticAsset = assets.find((asset) => asset.fileName === automatic!.sourcemapFileName);
    expect(automaticAsset).toBeDefined();
    expect(JSON.parse(automaticAsset!.source as string)).toStrictEqual(
      JSON.parse(automatic!.map!.toString()),
    );

    expect(assets.some((asset) => asset.fileName === deleted!.sourcemapFileName)).toBe(false);
  },
});
