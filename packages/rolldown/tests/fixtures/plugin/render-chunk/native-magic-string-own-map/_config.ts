import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import { defineTest } from 'rolldown-tests';
import { getLocation, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// The native MagicString's `generateMap()` returns an object with getters for its fields.
// A `renderChunk` hook can return that object as its `map`, with the MagicString or with a
// string as its `code`.
export default defineTest({
  config: {
    input: ['main.js'],
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'return-magic-string-with-map',
        renderChunk(_code, _chunk, _options, meta) {
          const s = meta.magicString!;
          s.prepend('globalThis.first = true;\n');
          return { code: s, map: s.generateMap({ hires: 'boundary' }) };
        },
      },
      {
        name: 'return-string-with-map',
        renderChunk(_code, _chunk, _options, meta) {
          const s = meta.magicString!;
          s.prepend('globalThis.second = true;\n');
          return { code: s.toString(), map: s.generateMap({ hires: 'boundary' }) };
        },
      },
    ],
  },
  afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code).toContain('globalThis.first');
    expect(chunk.code).toContain('globalThis.second');

    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    const expectedMappings = [
      ['parseFloat', { line: 1, column: 14 }],
      ['isNaN', { line: 2, column: 12 }],
    ] as const;

    for (const [token, expected] of expectedMappings) {
      expect(chunk.code).toContain(token);
      const original = originalPositionFor(map, getLocation(chunk.code, token));
      expect(original, `Incorrect original position for ${JSON.stringify(token)}`).toMatchObject({
        source: expect.stringMatching(/main\.js$/),
        ...expected,
      });
    }
  },
});
