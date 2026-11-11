import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import fs from 'node:fs';
import path from 'node:path';
import { RolldownMagicString } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getLocation, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// Each hook adds this text at the start of line 1, so the code after it moves.
const inserted = '/* inserted */ ';

// The native MagicString's `generateMap()` returns an object with getters for its fields.
// A `load` or a `transform` hook can return it as its `map`. When a `transform` hook also
// returns the MagicString as its `code`, Rolldown uses the hook's `map` and does not generate
// a second map from the MagicString.
export default defineTest({
  config: {
    input: ['main.js'],
    experimental: {
      // With `output.sourcemap`, this also starts the native sourcemap worker.
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'load-with-native-map',
        load(id) {
          if (path.basename(id) !== 'loaded.js') {
            return;
          }
          const s = new RolldownMagicString(fs.readFileSync(id, 'utf8'));
          s.prepend(inserted);
          return {
            code: s.toString(),
            map: s.generateMap({ hires: 'boundary', source: 'loaded.js', includeContent: true }),
          };
        },
      },
      {
        name: 'transform-with-native-map',
        transform(_code, id, meta) {
          const name = path.basename(id);
          if (!['boundary.js', 'line.js', 'null-map.js', 'auto.js'].includes(name)) {
            return;
          }
          const s = meta.magicString!;
          s.prepend(inserted);
          switch (name) {
            case 'boundary.js':
              return { code: s, map: s.generateMap({ hires: 'boundary' }) };
            case 'line.js':
              return { code: s, map: s.generateMap({ hires: false }) };
            case 'null-map.js':
              return { code: s, map: null };
            default:
              return { code: s };
          }
        },
      },
    ],
  },
  afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    // Each module has one line, and its token starts at column 12 of the original code.
    const expectedMappings = [
      ['parseFloat', 'boundary.js', 12],
      // The hook's map is line-level, so the token resolves to the start of its line.
      ['parseInt', 'line.js', 0],
      // `map: null` tells Rolldown that the step did not move code, so Rolldown does not
      // remap the positions of this step. The column includes the inserted text.
      ['isNaN', 'null-map.js', 12 + inserted.length],
      // The hook returns no `map`, so Rolldown generates one from the MagicString.
      ['isFinite', 'auto.js', 12],
      ['encodeURI', 'loaded.js', 12],
    ] as const;

    for (const [token, file, column] of expectedMappings) {
      expect(chunk.code).toContain(token);
      const original = originalPositionFor(map, getLocation(chunk.code, token));
      expect(original, `Incorrect original position for ${JSON.stringify(token)}`).toMatchObject({
        line: 1,
        column,
      });
      expect(path.basename(original.source ?? '')).toBe(file);
    }
  },
});
