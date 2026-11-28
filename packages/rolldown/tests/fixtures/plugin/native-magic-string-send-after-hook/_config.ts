import { RolldownMagicString } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// `sendMagicString` is public on the context impl but not on the declared interface.
type SendMagicStringContext = { sendMagicString(s: RolldownMagicString): void };

// The plugin driver closes the map slot of a context right after the hook, also when the
// hook returns `null`. Then the context keeps no map alive, and a later `sendMagicString`
// through a context that a plugin keeps fails instead of storing a map that nothing reads.
let kept: SendMagicStringContext | undefined;
let lateSendError: unknown;

export default defineTest({
  // WASI generates sourcemaps inline without a sourcemap worker, so `sendMagicString`
  // returns JSON directly instead of storing into the pending-sourcemap slot.
  skip: isWasiTest,
  config: {
    input: ['main.js'],
    output: {
      // this starts the sourcemap worker, so a send stores the map in the slot instead
      // of returning it as JSON.
      sourcemap: true,
    },
    plugins: [
      {
        name: 'send-then-return-null',
        transform(code, id) {
          if (!id.endsWith('main.js')) {
            return null;
          }
          kept = this as unknown as SendMagicStringContext;
          const s = new RolldownMagicString(code);
          s.prepend('// sent\n');
          kept.sendMagicString(s);
          return null;
        },
      },
      {
        name: 'send-through-the-kept-context',
        transform(code, id) {
          if (!id.endsWith('main.js')) {
            return null;
          }
          // A fresh MagicString, so the error cannot come from the check for a MagicString
          // that was already sent.
          try {
            kept!.sendMagicString(new RolldownMagicString(code));
          } catch (error) {
            lateSendError = error;
          }
          return null;
        },
      },
    ],
  },
  afterTest() {
    expect(String(lateSendError)).toMatch(
      /`sendMagicString` can only be called during the transform hook/,
    );
  },
});
