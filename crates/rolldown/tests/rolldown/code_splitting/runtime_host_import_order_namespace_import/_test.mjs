import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

// `namespace.js` imports `init.js` through the `init-barrel` package, which has no side effects, and
// reads `marker.initValue()` from the namespace. The import order walk does not reach `init.js`
// through that read, so the chunk of `init.js` keeps its `exec_order` position in the imports of
// `c`. The chunk of `common.js` is the bitset host and the dominator of the runtime consumers. With
// the runtime inside, it would get `exec_order` 0 and move before the chunk of `init.js`, and
// `common.js` would read the global before `init.js` sets it. So the runtime stays in its own chunk.
// A fresh process, because `init.js` sets a global that `common.js` reads at load time.
const entry = JSON.stringify(new URL('./dist/c.js', import.meta.url).href);
const script = `const { c } = await import(${entry}); console.log(JSON.stringify(c.value));`;
const value = JSON.parse(
  execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
);
assert.equal(value, 'ready');
