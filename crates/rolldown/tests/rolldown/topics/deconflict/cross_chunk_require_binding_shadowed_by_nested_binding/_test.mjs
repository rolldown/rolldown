import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Pre-fix, `require_shared.shared()` read the parameter:
// `TypeError: require_shared.shared is not a function`.
const { read } = createRequire(import.meta.url)('./dist/main.js');

assert.deepEqual(read('param'), ['shared', 'param']);
