import fs from 'node:fs';
import path from 'node:path';
import { originalPositionFor, sourceContentFor, TraceMap } from '@jridgewell/trace-mapping';
import type { ExistingRawSourceMap } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// A `load` hook sourcemap resolves each named source against the module's directory. An unnamed
// source (`null`) does not stop that and does not throw. Every module has two lines; `AAAA;ACAA`
// maps the first line to `sources[0]` and the second line to `sources[1]`.

const sources: Record<string, (string | null)[]> = {
  'unnamed-first.js': [null, 'first.ts'],
  'unnamed-second.js': ['second.ts', null],
};

function locate(code: string, text: string) {
  const offset = code.search(new RegExp(`["']${text}["']`));
  expect(offset, `${text} is not in the output`).toBeGreaterThanOrEqual(0);
  const precedingLines = code.slice(0, offset).split('\n');
  return { line: precedingLines.length, column: precedingLines.at(-1)!.length };
}

export default defineTest({
  config: {
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'load-map-with-unnamed-source',
        load(id) {
          const names = sources[path.basename(id)];
          if (!names) {
            return;
          }
          const map: ExistingRawSourceMap = {
            version: 3,
            names: [],
            sources: names,
            sourcesContent: names.map((name) => name ?? 'unnamed'),
            mappings: 'AAAA;ACAA',
          };
          // Rolldown must not change the map that a plugin returns, so a write to it throws.
          Object.freeze(names);
          return { code: fs.readFileSync(id, 'utf8'), map: Object.freeze(map) };
        },
      },
    ],
  },
  afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    const origin = (text: string) => {
      const { source } = originalPositionFor(map, locate(chunk.code, text));
      return { source, content: source === null ? undefined : sourceContentFor(map, source) };
    };

    expect(origin('p1')).toMatchObject({ content: 'unnamed' });
    expect(origin('p2')).toEqual({ source: '../first.ts', content: 'first.ts' });
    expect(origin('q1')).toEqual({ source: '../second.ts', content: 'second.ts' });
    expect(origin('q2')).toMatchObject({ content: 'unnamed' });
  },
});
