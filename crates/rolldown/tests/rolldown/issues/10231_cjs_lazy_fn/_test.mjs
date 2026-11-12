import { createRequire } from 'node:module';
import assert from 'node:assert';
const require = createRequire(import.meta.url);
const fn = require('./dist/main.js');
assert.deepStrictEqual(globalThis.lazyLog, undefined, 'dep.cjs ran before the function was called');
assert.strictEqual(fn(), 'dep');
assert.deepStrictEqual(globalThis.lazyLog, ['dep ran']);
