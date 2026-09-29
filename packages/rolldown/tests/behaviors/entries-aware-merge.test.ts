import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';
import { getOutputChunk } from '../src/utils';

async function build(loadOrder: string[]) {
  const release = new Map<string, () => void>();
  const ready = new Map(
    loadOrder.map((name) => [
      name,
      new Promise<void>((resolve) => {
        release.set(name, resolve);
      }),
    ]),
  );
  release.get(loadOrder[0])!();

  const loaded: string[] = [];
  const bundle = await rolldown({
    input: Object.fromEntries(['a', 'b', 'c', 'd'].map((name) => [name, `entry:${name}`])),
    plugins: [
      {
        name: 'controlled-module-loading',
        resolveId(id) {
          return id;
        },
        async load(id) {
          const name = id.slice(-1);
          if (id.startsWith('entry:')) {
            await ready.get(name);
            return `import { icon } from 'icon:${name}'; console.log(icon());`;
          }

          loaded.push(name);
          // Loading this dependency means its module index has already been allocated.
          const next = loadOrder[loadOrder.indexOf(name) + 1];
          release.get(next)?.();
          return `export function icon() { return ['${name}']; }`;
        },
      },
    ],
  });

  try {
    const output = await bundle.generate({
      entryFileNames: 'entry-[name]-[hash].js',
      chunkFileNames: 'chunk-[hash].js',
      codeSplitting: {
        groups: [
          {
            name: 'icons',
            test: /^icon:/,
            entriesAware: true,
            entriesAwareMergeThreshold: 60,
          },
        ],
      },
    });
    expect(loaded).toEqual(loadOrder);
    const chunks = getOutputChunk(output);
    expect(chunks.filter((chunk) => !chunk.isEntry)).toHaveLength(2);
    return Object.fromEntries(chunks.map((chunk) => [chunk.fileName, chunk.code]));
  } finally {
    await bundle.close();
  }
}

// https://github.com/rolldown/rolldown/issues/10886
test('entries-aware merges are independent of module loading order', async () => {
  const expected = await build(['a', 'b', 'c', 'd']);
  for (const loadOrder of [
    ['a', 'c', 'b', 'd'],
    ['a', 'd', 'b', 'c'],
  ]) {
    expect(await build(loadOrder)).toEqual(expected);
  }
});
