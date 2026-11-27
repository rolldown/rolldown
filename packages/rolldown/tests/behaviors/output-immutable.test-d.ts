import { defineConfig, type Plugin, rolldown } from 'rolldown';
import { test } from 'vitest';

test('generated chunks and assets have a readonly immutable flag', async () => {
  const bundle = await rolldown({ input: 'entry.js' });
  const {
    output: [chunk, ...rest],
  } = await bundle.generate();
  // @ts-expect-error -- generated chunks expose immutable through a getter
  chunk.immutable = false;

  const asset = rest.find((file) => file.type === 'asset');
  if (asset) {
    // @ts-expect-error -- generated assets expose immutable through a getter
    asset.immutable = false;
  }
});

test('generateBundle can set immutable on chunks and assets', () => {
  defineConfig({
    plugins: [
      {
        name: 'set-immutable',
        generateBundle(_options, bundle) {
          const files = Object.values(bundle);
          const chunk = files.find((file) => file.type === 'chunk');
          if (chunk) chunk.immutable = false;

          const asset = files.find((file) => file.type === 'asset');
          if (asset) asset.immutable = false;
        },
      } satisfies Plugin,
    ],
  });
});
