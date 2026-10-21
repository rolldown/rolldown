// Reports which path native magic-string sourcemaps took. Calls the binding's
// `sendMagicString` (`this.inner`) because the public wrapper drops its return:
// `null` = sent to the sourcemap thread, a JSON string = generated inline on the
// JS thread.
import { build, RolldownMagicString } from 'rolldown';

const MODULE_COUNT = 4;
const ID_PREFIX = '\0sourcemap-offload-thread:';
const ENTRY_ID = `${ID_PREFIX}entry`;
const moduleId = (index) => `${ID_PREFIX}m${index}`;

let offloaded = 0;
let inline = 0;

const probePlugin = {
  name: 'sourcemap-offload-thread-probe',
  resolveId(id) {
    return id.startsWith(ID_PREFIX) ? id : null;
  },
  load(id) {
    if (id === ENTRY_ID) {
      return Array.from(
        { length: MODULE_COUNT },
        (_unused, index) => `export * from '${moduleId(index)}';`,
      ).join('\n');
    }
    const match = /^\0sourcemap-offload-thread:m(\d+)$/.exec(id);
    return match === null ? null : `export const value${match[1]} = ${match[1]};`;
  },
  transform(code, id) {
    const magicString = new RolldownMagicString(code);
    magicString.append('\n// probed\n');
    // `sendMagicString` consumes the magic string, so the code is built by hand.
    const rawMap = this.inner.sendMagicString(magicString);
    // Count only this probe's modules: `\0rolldown/runtime.js` is always inline.
    if (id.startsWith(ID_PREFIX)) {
      if (rawMap == null) {
        offloaded += 1;
      } else {
        inline += 1;
      }
    }
    return { code: `${code}\n// probed\n`, map: rawMap ?? null };
  },
};

await build({
  input: ENTRY_ID,
  plugins: [probePlugin],
  experimental: { nativeMagicString: true },
  output: { sourcemap: true },
  write: false,
});

console.log(
  JSON.stringify({
    offloaded,
    inline,
    expected: MODULE_COUNT + 1,
  }),
);
