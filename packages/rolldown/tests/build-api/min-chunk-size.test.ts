import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const modules: Record<string, string> = {
  '/page-a.js': "import { common1 } from '/common1.js'; console.log('page-a', common1);",
  '/page-b.js':
    "import { common1 } from '/common1.js'; import { common2 } from '/common2.js'; console.log('page-b', common1, common2);",
  '/page-c.js': "import { common2 } from '/common2.js'; console.log('page-c', common2);",
  '/common1.js': "export const common1 = 'common1 marker';",
  '/common2.js': "export const common2 = 'common2 marker';",
};

const virtualPlugin: Plugin = {
  name: 'virtual-min-chunk-size',
  resolveId(id) {
    if (id in modules) return id;
  },
  load(id) {
    return modules[id];
  },
};

async function build(minChunkSize: number) {
  const bundle = await rolldown({
    input: ['/page-a.js', '/page-b.js', '/page-c.js'],
    plugins: [virtualPlugin],
    experimental: {
      chunkOptimization: {
        minChunkSize,
      },
    },
  });
  const output = await bundle.generate({ format: 'esm' });
  await bundle.close();
  return output.output.filter((chunk): chunk is OutputChunk => chunk.type === 'chunk');
}

test('experimental.chunkOptimization.minChunkSize is passed through the JS API and NAPI binding', async () => {
  const disabledChunks = await build(0);
  const enabledChunks = await build(10_000);

  expect(disabledChunks).toHaveLength(5);
  expect(enabledChunks).toHaveLength(4);
  expect(
    enabledChunks.filter(
      (chunk) => chunk.code.includes('common1 marker') && chunk.code.includes('common2 marker'),
    ),
  ).toHaveLength(1);
});
