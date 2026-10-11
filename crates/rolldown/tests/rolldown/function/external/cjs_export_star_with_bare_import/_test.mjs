import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const main = require('./dist/main.js');
assert.strictEqual(main.value, 'ext');
