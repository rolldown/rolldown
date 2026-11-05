import assert from 'node:assert';
import { log, value } from './dist/main.js';

assert.strictEqual(value, 'd');
// `import.defer()` evaluates the top-level await of `t.js` before it resolves, but not in the call.
assert.deepStrictEqual(log, ['after call', 't start', 't end', 'after import', 'd']);
