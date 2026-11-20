import assert from 'node:assert';
import { log, same, values } from './dist/main.js';

assert.deepStrictEqual(values, ['d', 'd']);
assert.strictEqual(same, false);
// `import()` evaluates `d.js`, and `import.defer()` does not.
assert.deepStrictEqual(log, ['after import.defer', 'd', 'after import']);
