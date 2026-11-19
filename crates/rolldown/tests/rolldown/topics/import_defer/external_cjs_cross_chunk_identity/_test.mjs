import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

const main = require('./dist/main.js');
const other = require('./dist/other.js');
assert.strictEqual(main.lazyNamespace(), other.c);
assert.strictEqual(other.c.y, 7);
