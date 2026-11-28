import assert from 'node:assert';
import * as m from './dist/main.js';

// `experimental.importDefer` has no effect yet, so `dep.js` evaluates before `main.js`.
assert.deepStrictEqual(m.before, ['dep']);
assert.strictEqual(m.value, 1);
assert.deepStrictEqual(m.after, ['dep']);
