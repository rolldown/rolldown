import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// An entry that imports `count` modules dynamically, so the build has one chunk per module.
function dynamicImports(
  count: number,
  marked: (index: number) => boolean,
  length: (index: number) => number = (index) => (index * 37) % 500,
): Plugin {
  return {
    name: 'dynamic-imports',
    resolveId(id) {
      if (id === 'entry' || /^m\d+$/.test(id)) return id;
    },
    load(id) {
      if (id === 'entry') {
        return Array.from(
          { length: count },
          (_, i) => `export const l${i} = () => import('m${i}');`,
        ).join('\n');
      }
      const index = Number(id.slice(1));
      const value = (marked(index) ? 'MARK' : '') + 'x'.repeat(length(index));
      return `export const v${index} = ${JSON.stringify(value)};`;
    },
  };
}

test('a renderChunk hook after a filtered one sees the chunks in the same order on every build', async () => {
  async function build() {
    const calls: string[] = [];
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        dynamicImports(200, (index) => index % 5 === 0),
        {
          name: 'filtered',
          renderChunk: {
            filter: { code: /MARK/ },
            handler: (code) => `${code}\n// filtered`,
          },
        },
        {
          name: 'recorder',
          renderChunk(_code, chunk) {
            calls.push(chunk.name);
            return null;
          },
        },
      ],
    });
    const { output } = await bundle.generate({});
    await bundle.close();
    const chunks = output.filter((item): item is OutputChunk => item.type === 'chunk');
    return { calls, chunks };
  }

  const first = await build();
  // Every chunk starts at once, so `recorder` first sees the chunks that `filtered` skips, then
  // the others, each in chunk order.
  const skipped = first.chunks.filter((chunk) => !chunk.code.includes('MARK'));
  const filtered = first.chunks.filter((chunk) => chunk.code.includes('MARK'));
  expect(filtered).toHaveLength(40);
  expect(first.calls).toEqual([...skipped, ...filtered].map((chunk) => chunk.name));
  for (let run = 0; run < 5; run++) {
    expect((await build()).calls).toEqual(first.calls);
  }
});

test('a renderChunk hook can wait until every chunk has called it', async () => {
  const count = 300;
  let calls = 0;
  let release!: () => void;
  const everyChunkCalled = new Promise<void>((resolve) => {
    release = resolve;
  });
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      dynamicImports(count, () => false),
      {
        name: 'wait-for-every-chunk',
        async renderChunk() {
          calls++;
          if (calls === count + 1) release();
          await everyChunkCalled;
          return null;
        },
      },
    ],
  });
  const { output } = await bundle.generate({});
  await bundle.close();
  expect(output.filter((item) => item.type === 'chunk')).toHaveLength(count + 1);
});

test('with more than 16 chunks, a chunk holding more than a sixteenth of the code reaches renderChunk first', async () => {
  const calls: string[] = [];
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      dynamicImports(
        40,
        () => false,
        (index) => (index === 30 ? 100_000 : 100),
      ),
      {
        name: 'recorder',
        renderChunk(_code, chunk) {
          calls.push(chunk.name);
          return null;
        },
      },
    ],
  });
  const { output } = await bundle.generate({});
  await bundle.close();
  const others = output
    .filter((item): item is OutputChunk => item.type === 'chunk')
    .map((chunk) => chunk.name)
    .filter((name) => name !== 'm30');
  expect(calls).toEqual(['m30', ...others]);
});
