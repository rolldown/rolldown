import assert from 'node:assert';
import fs from 'node:fs';
// Each lone surrogate in a request becomes U+FFFD, as it does when Node loads the module, so
// `./a\uD800.js` names the file `a\u{FFFD}.js`.
import a from './a\uD800.js';
export * from './b\uD800.js';
export { c } from './c\uD800.js';

assert.strictEqual(a, 'a');
assert.strictEqual((await import('./d\uD800.js')).d, 'd');
assert.strictEqual(require(`./e\uD800.cjs`), 'e');
let f;
try {
  f = require('./f\uD800.cjs');
} catch {}
assert.strictEqual(f, 'f');
assert.ok(fs.existsSync(new URL('./g\uD800.bin', import.meta.url)));

// An ignored request and a data URL are left as written.
export const ignored = () => import(/* @vite-ignore */ './h\uD800.js');
export const ignoredUrl = new URL(/* @vite-ignore */ './i\uD800.bin', import.meta.url);
export const dataUrl = new URL('data:text/plain,\uD800', import.meta.url);
