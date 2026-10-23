import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
assert.deepEqual(require('./dist/main.js'), require('./lib.cjs'));
