import assert from 'node:assert';
import { s_helper, s_task } from './dist/group.js';
import { load } from './dist/index.js';

assert.strictEqual(s_task(), 'task');
assert.strictEqual(s_helper(), 'helper');
assert.strictEqual((await load()).s_task(), 'task');
