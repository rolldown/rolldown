import assert from 'node:assert/strict';
import { loadDirect, loadOther } from './dist/main.js';

assert.deepEqual((await loadDirect()).group(), { name: 'Group' });
assert.deepEqual((await loadOther()).check(), {});
