import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
globalThis.order = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(globalThis.order, ['ext', 'main']);
assert.strictEqual(main.value, 'ext');
