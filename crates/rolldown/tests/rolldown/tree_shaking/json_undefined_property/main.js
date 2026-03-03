import assert from 'node:assert';
import foo from './foo.json';
assert.deepEqual(foo.flat(), [1, 2]);
