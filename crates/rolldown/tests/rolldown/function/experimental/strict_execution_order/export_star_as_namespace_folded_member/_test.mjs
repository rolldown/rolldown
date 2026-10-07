import assert from 'node:assert/strict';
import { load, loadOther } from './dist/main.js';

assert.deepEqual((await load()).check(), [{ name: 'Group' }, { name: 'Item' }]);
assert.deepEqual((await loadOther()).check(), {});
