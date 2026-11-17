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

test('a renderChunk hook after a filtered one sees every chunk once', async () => {
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
  expect(chunks.filter((chunk) => chunk.code.includes('MARK'))).toHaveLength(40);
  // Chunks call their hooks concurrently, so the order of the calls is not part of the contract.
  expect([...calls].sort()).toEqual(chunks.map((chunk) => chunk.name).sort());
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
  const names = output
    .filter((item): item is OutputChunk => item.type === 'chunk')
    .map((chunk) => chunk.name);
  expect(calls[0]).toBe('m30');
  expect([...calls].sort()).toEqual([...names].sort());
});
