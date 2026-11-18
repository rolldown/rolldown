import assert from 'node:assert/strict';
import { sep } from 'node:path';
import { createRequire } from 'node:module';

const load =
  globalThis.__configName === 'Cjs' ? createRequire(import.meta.url) : (id) => import(id);
const group = await load('./dist/group.js');
for (const name of ['s_task', 's_foo', 's_sep', 'ns', 'v', 'w', 'x', 'y']) {
  assert.ok(Object.hasOwn(group, name), name);
}
assert.equal(group.s_task(), 'task');
assert.equal(group.s_foo(), 'foo');
assert.equal(group.s_sep, sep);
assert.equal(group.ns.sep, sep);
assert.equal(group.v, group.w);
assert.equal(group.v(), 'v');
assert.equal(group.x, group.y);
assert.equal(group.x(), 'x');
