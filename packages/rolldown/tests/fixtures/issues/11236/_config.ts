import path from 'node:path';
import { originalPositionFor, sourceContentFor, TraceMap } from '@jridgewell/trace-mapping';
import type { ExistingRawSourceMap } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// https://github.com/rolldown/rolldown/issues/11236: each source of a `transform` hook sourcemap
// keeps its own content. Every module has two lines; `AAAA;ACAA` maps the first line to
// `sources[0]` and the second line to `sources[1]`.

const src = (name: string) => path.join(import.meta.dirname, 'src', name);

/** Rolldown must not change the map that a plugin returns, so a write to it throws. */
function deepFreeze<T extends object>(value: T): T {
  for (const child of Object.values(value)) {
    if (child && typeof child === 'object') {
      deepFreeze(child);
    }
  }
  Object.freeze(value);
  return value;
}

function map(
  sources: (string | null)[],
  sourcesContent?: (string | null)[],
  mappings = 'AAAA;ACAA',
): ExistingRawSourceMap {
  return deepFreeze({ version: 3, names: [], sources, sourcesContent, mappings });
}

const maps: Record<string, (id: string) => ExistingRawSourceMap | string> = {
  'empty-content.js': () => map([src('a.ts'), src('b.ts')], ['', 'export const b = 2']),
  'null-content.js': () => map([src('c.ts'), src('d.ts')], [null, 'export const d = 4']),
  'no-content.js': () => map([src('e.ts'), src('f.ts')]),
  'unnamed-source.js': () => map(['', src('h.ts')], ['G', 'H']),
  // Only the entry named with the module id is the module, so only it gets the module's code.
  'module-source.js': (id) => map([src('i.ts'), id], ['I', '']),
  // A single source is the module itself, so it gets the module's code.
  'single-source.js': () => map([src('k.ts')], [''], 'AAAA;AACA'),
  'string-map.js': () => JSON.stringify(map([src('m.ts'), src('n.ts')], ['', 'N'])),
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
        name: 'multi-source-map',
        transform(code, id) {
          const createMap = maps[path.basename(id)];
          return createMap && { code, map: createMap(id) };
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
    const at = (file: string) => expect.stringMatching(new RegExp(`/${file.replace('.', '\\.')}$`));

    expect(origin('a1')).toEqual({ source: at('src/a.ts'), content: '' });
    expect(origin('a2')).toEqual({ source: at('src/b.ts'), content: 'export const b = 2' });
    expect(origin('c1')).toEqual({ source: at('src/c.ts'), content: null });
    expect(origin('c2')).toEqual({ source: at('src/d.ts'), content: 'export const d = 4' });
    expect(origin('e1')).toEqual({ source: at('src/e.ts'), content: null });
    expect(origin('e2')).toEqual({ source: at('src/f.ts'), content: null });
    expect(origin('g1')).toMatchObject({ content: 'G' });
    expect(origin('g2')).toEqual({ source: at('src/h.ts'), content: 'H' });
    expect(origin('i1')).toEqual({ source: at('src/i.ts'), content: 'I' });
    expect(origin('i2')).toEqual({
      source: at('module-source.js'),
      content: "console.log('i1');\nconsole.log('i2');\n",
    });
    const singleSource = "console.log('k1');\nconsole.log('k2');\n";
    expect(origin('k1')).toEqual({ source: at('single-source.js'), content: singleSource });
    expect(origin('k2')).toEqual({ source: at('single-source.js'), content: singleSource });
    expect(origin('m1')).toEqual({ source: at('src/m.ts'), content: '' });
    expect(origin('m2')).toEqual({ source: at('src/n.ts'), content: 'N' });
  },
});
