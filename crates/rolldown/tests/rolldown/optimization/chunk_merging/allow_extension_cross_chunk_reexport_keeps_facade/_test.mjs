import assert from 'node:assert';
import { s_local } from './dist/group.js';
import { load } from './dist/index.js';
import { s_task } from './dist/seg_dynamic.js';
import { s_helper } from './dist/seg_static.js';

assert.strictEqual(s_task(), 'task');
assert.strictEqual(s_helper(), 'helper');
assert.strictEqual(s_local(), 'local');
assert.strictEqual((await load()).s_task(), 'task');
