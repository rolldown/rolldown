import path from 'node:path';
import { build } from 'rolldown';
import type { ExistingRawSourceMap } from 'rolldown';
import { expect, test } from 'vitest';

test.each([
  ['load', null, 'original.js'],
  ['load', undefined, 'original.js'],
  ['load', 'original.js', undefined],
  ['transform', null, 'original.js'],
  ['transform', undefined, 'original.js'],
  ['transform', 'original.js', undefined],
] as const)('%s maps preserve source slots [%o, %o]', async (hook, first, second) => {
  const entry = path.join(import.meta.dirname, 'entry.js');
  const code = 'export const first = 1;\nexport const answer = 42;';
  const sources = [first, second];
  const sourcesContent = sources.map((source) => (source == null ? null : code));
  const map: ExistingRawSourceMap = {
    version: 3,
    sources,
    sourcesContent,
    sourceRoot: 'src',
    names: [],
    mappings: 'AAAA;ACAA',
  };
  const result = await build({
    input: entry,
    plugins: [
      {
        name: 'nullable-sourcemap',
        resolveId(id) {
          if (id === entry) return id;
        },
        load(id) {
          if (id === entry) return hook === 'load' ? { code, map } : code;
        },
        transform(source, id) {
          if (id === entry && hook === 'transform') return { code: source, map };
        },
      },
    ],
    output: { dir: import.meta.dirname, sourcemap: true },
    write: false,
  });
  const chunk = result.output.find((item) => item.type === 'chunk')!;
  const original = hook === 'load' ? 'src/original.js' : expect.stringMatching(/original\.js$/);
  expect(chunk.map!.sources).toEqual(sources.map((source) => (source == null ? null : original)));
  expect(chunk.map!.sourcesContent).toEqual(sourcesContent);
});
