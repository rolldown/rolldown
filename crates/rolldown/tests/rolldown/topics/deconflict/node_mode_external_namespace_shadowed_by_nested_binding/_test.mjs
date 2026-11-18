import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Before the fix, the node-mode namespace took the name `node_https$1`, the parameter captured
// it, and `read` threw `Cannot read properties of undefined (reading 'request')`.
const require = createRequire(import.meta.url);
const main = require('./dist/main.js');
assert.deepEqual(main.read('param'), ['function', 'param', 'function']);
assert.equal(main.value, 'function');
