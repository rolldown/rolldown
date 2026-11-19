import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const main = require('./dist/main.js');

main.change();
assert.deepStrictEqual(main.read(), [false, false]);
assert.strictEqual(main.flag, false);
assert.strictEqual(main.other, false);
