import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import path from 'node:path';
import { RolldownMagicString } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getLocation, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// The hook adds this text at the start of line 1, so the code after it moves.
const inserted = '/* inserted */ ';

// The `transform` hook returns a MagicString without a `map`, so Rolldown generates the map.
// It uses the `hires` option of the MagicString, or `'boundary'` when the option is not set.
// `meta.magicString` has no options, so the hook creates the MagicString itself.
export default defineTest({
  config: {
    input: ['main.js'],
    experimental: {
      // With `output.sourcemap`, the native sourcemap worker generates the map.
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'transform-with-hires',
        transform(code, id) {
          const name = path.basename(id);
          if (name !== 'unset.js' && name !== 'line.js') {
            return;
          }
          const s = new RolldownMagicString(code, name === 'line.js' ? { hires: false } : {});
          s.prepend(inserted);
          return { code: s };
        },
      },
    ],
  },
  afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    // Each module has one line, and its token starts at column 12 of the original code.
    const expectedMappings = [
      ['parseFloat', 'unset.js', 12],
      // The map is line-level, so the token resolves to the start of its line.
      ['parseInt', 'line.js', 0],
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
