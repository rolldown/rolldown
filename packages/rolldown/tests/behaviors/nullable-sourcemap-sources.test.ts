import path from 'node:path';
import { build } from 'rolldown';
import type { ExistingRawSourceMap } from 'rolldown';
import { expect, test } from 'vitest';

test.each(['load', 'transform'] as const)('%s maps preserve null source slots', async (hook) => {
  const entry = path.join(import.meta.dirname, 'entry.js');
  const code = 'export const first = 1;\nexport const answer = 42;';
  const map: ExistingRawSourceMap = {
    version: 3,
    sources: [null, 'original.js'],
    sourcesContent: [null, code],
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
  expect(chunk.map!.sources[0]).toBeNull();
  expect(chunk.map!.sources[1]).toMatch(/original\.js$/);
  expect(chunk.map!.sourcesContent).toEqual([null, code]);
  if (hook === 'load') expect(chunk.map!.sources[1]).toBe('src/original.js');
});
