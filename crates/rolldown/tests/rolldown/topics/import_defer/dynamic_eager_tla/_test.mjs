import assert from 'node:assert';
import { log, value } from './dist/main.js';

assert.strictEqual(value, 'd');
// The eager `import()` resolves after the top-level await of `t.js` and the evaluation of `d.js`.
assert.deepStrictEqual(log, ['t start', 't end', 'd', 'after import']);
