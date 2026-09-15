import { defineTest } from 'rolldown-tests';
import type { RolldownMagicString } from 'rolldown';
import { expect } from 'vitest';

// `sendMagicString` moves the MagicString's contents out to the native sourcemap channel.
// Every read and edit on the consumed instance refuses, but the transfer itself was left
// unguarded: a second send returned normally and queued the empty MagicString left behind
// by the first move, silently replacing the real map in the channel.
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
        name: 'test-magic-string-double-send',
        transform(code, id, meta) {
          if (id.startsWith('\0') || !meta?.magicString) {
            return null;
          }
          const ms = meta.magicString;
          ms.append('\nconsole.log("appended");');
          const out = ms.toString();
          // `sendMagicString` is public on the context impl but not on the declared
          // interface; any JS plugin can reach it, so the runtime path is what matters.
          const ctx = this as unknown as {
            sendMagicString(s: RolldownMagicString): void;
          };
          // First transfer: the legitimate handoff to the native sourcemap channel.
          ctx.sendMagicString(ms);
          // Repeating the transfer must refuse like every other consumed-instance API.
          expect(() => ctx.sendMagicString(ms)).toThrow(/already passed to `sendMagicString/);
          // `map: null` signals the map was delivered out-of-band via the channel;
          // omitting it would mark this transform Omitted and wipe the channel map.
          return { code: out, map: null };
        },
      },
    ],
  },
  afterTest: function (output) {
    // The first send and the transform result itself still work.
    const chunk = output.output[0];
    expect(chunk.code).toContain('appended');
    // The first transfer's map survived the rejected second one.
    expect(chunk.map).toBeTruthy();
    expect(chunk.map!.mappings).not.toBe('');
  },
});
