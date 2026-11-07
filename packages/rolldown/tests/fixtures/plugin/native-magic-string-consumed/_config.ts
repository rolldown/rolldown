import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import { defineTest } from 'rolldown-tests';
import type { RolldownMagicString } from 'rolldown';
import { expect } from 'vitest';

// Returning a MagicString from `transform` hands it to `sendMagicString`, which moves its
// contents out to the native sourcemap channel. The JS object outlives that move, so a
// plugin holding on to the reference used to get an empty MagicString back with no error
// — silently producing empty code. It must refuse instead.
let stashed: RolldownMagicString | undefined;

export default defineTest({
  config: {
    input: ['main.js'],
    experimental: { nativeMagicString: true },
    output: { sourcemap: true },
    plugins: [
      {
        name: 'test-magic-string-consumed',
        transform(code, id, meta) {
          if (id.startsWith('\0') || !meta?.magicString) {
            return null;
          }
          stashed = meta.magicString;
          stashed.append('\nconsole.log("appended");');
          // Still live here — the handoff happens when we return it.
          expect(stashed.toString()).toContain('appended');
          return { code: stashed };
        },
        buildEnd() {
          expect(stashed).toBeDefined();
          const consumed = /already passed to `sendMagicString/;
          // Reads and edits alike must throw rather than report an empty string.
          expect(() => stashed!.toString()).toThrow(consumed);
          expect(() => stashed!.length()).toThrow(consumed);
          expect(() => stashed!.hasChanged()).toThrow(consumed);
          expect(() => stashed!.generateMap({})).toThrow(consumed);
          expect(() => stashed!.append('x')).toThrow(consumed);
          expect(() => stashed!.overwrite(0, 1, 'x')).toThrow(consumed);

          // Sending leaves the UTF-16 mapper behind; it can still be explicitly released.
          expect(stashed!.dropInner().freed).toBe(true);
          expect(stashed!.dropInner()).toEqual({
            freed: false,
            reason: 'Memory has already been freed',
          });
          expect(() => stashed!.toString()).toThrow(consumed);
          expect(() => stashed!.append('x')).toThrow(consumed);
        },
      },
    ],
  },
  afterTest: function (output) {
    // Releasing the wrapper must not invalidate the code or map already sent to the worker.
    const chunk = output.output[0];
    expect(chunk.code).toContain('hello');
    expect(chunk.code).toContain('appended');
    expect(chunk.map).toBeTruthy();
    expect(chunk.map!.sourcesContent).toEqual(["console.log('hello');\n"]);
    const lines = chunk.code.split('\n');
    const line = lines.findIndex((line) => line.includes('hello'));
    const map = new TraceMap(JSON.parse(chunk.map!.toString()));
    expect(
      originalPositionFor(map, { line: line + 1, column: lines[line].indexOf('console') }),
    ).toMatchObject({ line: 1, column: 0 });
  },
});
