import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import { RolldownMagicString } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getLocation, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// The `hires` option for each entry chunk. The three entry modules have the same code.
const hiresByChunk: Record<string, boolean | undefined> = { a: undefined, b: false, c: true };

// The `renderChunk` hook returns a MagicString without a `map`, so Rolldown generates the map.
// It uses the `hires` option of the MagicString, or `'boundary'` when the option is not set.
// `true` and `'boundary'` give the same positions here. With `true`, the output map also has a
// segment for each character, and each extra segment repeats the position before it.
export default defineTest({
  config: {
    input: { a: 'a.js', b: 'b.js', c: 'c.js' },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'render-chunk-with-hires',
        renderChunk(code, chunk) {
          const hires = hiresByChunk[chunk.name];
          const s = new RolldownMagicString(code, hires === undefined ? {} : { hires });
          s.prepend('globalThis.edited = true;\n');
          return s;
        },
      },
    ],
  },
  afterTest(output) {
    const exact = [
      ['parseFloat', { line: 1, column: 12 }],
      ['isNaN', { line: 2, column: 12 }],
    ] as const;
    // The map of chunk `b` is line-level, so each token resolves to the start of its line.
    const lineLevel = [
      ['parseFloat', { line: 1, column: 0 }],
      ['isNaN', { line: 2, column: 0 }],
    ] as const;
    const chunks = new Map(getOutputChunk(output).map((chunk) => [chunk.name, chunk]));
    for (const [name, expectedMappings] of [
      ['a', exact],
      ['b', lineLevel],
      ['c', exact],
    ] as const) {
      const chunk = chunks.get(name)!;
      expect(chunk.code).toContain('globalThis.edited');
      const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
      for (const [token, expected] of expectedMappings) {
        expect(chunk.code).toContain(token);
        const original = originalPositionFor(map, getLocation(chunk.code, token));
        expect(original, `Incorrect original position for ${token} in ${name}`).toMatchObject({
          source: expect.stringMatching(new RegExp(`${name}\\.js$`)),
          ...expected,
        });
      }
    }
  },
});
