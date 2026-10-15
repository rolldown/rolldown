import { createRequire } from 'node:module';
import assert from 'node:assert';

const require = createRequire(import.meta.url);
const dep = require('./dist/dep.js');

assert.strictEqual(require('./dist/entry.js').seen, 'value');
assert.strictEqual(typeof dep.join, 'function', 'external star re-export must stay reachable');
