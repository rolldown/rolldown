import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(globalThis.__extLog, ['main']);

// The first access evaluates `d.js`, which loads its externals in source order.
assert.deepStrictEqual(main.read(), [1, 2, 3]);
assert.deepStrictEqual(globalThis.__extLog, [
  'main',
  'ext-bare',
  'ext-named',
  'ext-ns',
  'ext-star',
  'd',
]);
assert.deepStrictEqual(main.keys(), ['nsValue', 'starValue', 'x']);
assert.strictEqual(globalThis.__extLog.length, 6);

delete globalThis.__extLog;
