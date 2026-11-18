import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const load =
  globalThis.__configName === 'Cjs' ? createRequire(import.meta.url) : (id) => import(id);
const group = await load('./dist/group.js');
const entry = await load('./dist/entry.js');
assert.equal(group.s_task, entry.task);
assert.equal(group.s_task(), 'task');
