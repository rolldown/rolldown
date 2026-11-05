import assert from 'node:assert';
import { keys, log, then, value } from './dist/main.js';

assert.strictEqual(value, 'd');
assert.deepStrictEqual(keys, ['value']);
assert.strictEqual(then, false);
// `import.defer()` does not evaluate `d.js`. `Object.keys` evaluates it.
assert.deepStrictEqual(log, ['after import', 'd', 'after keys']);
