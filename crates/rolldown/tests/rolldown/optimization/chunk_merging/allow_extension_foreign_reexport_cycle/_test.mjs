import assert from 'node:assert/strict';

const segment = await import('./dist/seg_task.js');
const entry = await import('./dist/entry.js');
assert.equal(segment.s_task, entry.task);
assert.equal(segment.s_task(), 'task');
