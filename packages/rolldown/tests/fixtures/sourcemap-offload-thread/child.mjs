// Reports which path the scan stage took for native magic-string sourcemaps.
//
// `BindingTransformPluginContext#sendMagicString` returns `null` when the map
// was handed to the dedicated sourcemap OS thread, and a JSON string when the
// map had to be generated inline, on the JavaScript host thread, inside the
// synchronous napi call. That raw return is the only direct evidence of which
// path ran, so this child calls the binding method (`ctx.inner`) rather than
// the public `void` wrapper, which discards it.
//
// The flavor lives in this child's own environment because `ROLLDOWN_RUNTIME`
// is read once, when the binding loads.
import { build, RolldownMagicString } from 'rolldown';
import { getRuntimeCapabilities } from 'rolldown/experimental';

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
    // `sendMagicString` consumes the magic string, so the transformed code is
    // assembled here rather than read back off it afterwards.
    const rawMap = this.inner.sendMagicString(magicString);
    // `\0rolldown/runtime.js` is transformed outside the scan stage's channel
    // window, so it is always inline on every flavor. Only the modules this
    // probe owns say anything about the gate.
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

const capabilities = getRuntimeCapabilities();
console.log(
  JSON.stringify({
    flavor: capabilities.flavor,
    threads: capabilities.threads,
    wasi: capabilities.wasi,
    offloaded,
    inline,
    expected: MODULE_COUNT + 1,
  }),
);
