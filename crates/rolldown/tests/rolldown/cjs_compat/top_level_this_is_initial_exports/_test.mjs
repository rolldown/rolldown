import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Top-level `this` of a CommonJS module is the exports object it starts with, whatever the module
// does to `exports`, `module` or `module.exports` afterwards. Each module is checked against what
// Node itself returns for it.
const require = createRequire(import.meta.url);
let all;
if (globalThis.__configName === 'cjs') {
  all = require('./dist/main.js').all;
} else if (globalThis.__configName === 'umd') {
  // `dist` is an ES module package, so the UMD wrapper takes its global branch.
  await import('./dist/main.js');
  all = globalThis.lib.all;
} else {
  all = (await import('./dist/main.js')).all;
}
for (const [name, value] of Object.entries(all)) {
  assert.deepEqual(value, require(`./${name}.cjs`), name);
}
