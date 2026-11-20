import assert from 'node:assert';
import * as m from './dist/main.js';

// `dep.js` evaluates at the first access to `dep.value`, after `before` reads the log.
assert.deepStrictEqual(m.before, []);
assert.strictEqual(m.value, 1);
assert.deepStrictEqual(m.after, ['dep']);
