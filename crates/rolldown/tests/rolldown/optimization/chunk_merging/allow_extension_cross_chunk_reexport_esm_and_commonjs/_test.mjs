import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  globalThis.__configName === 'cjs'
    ? require(`./dist/${name}.js`)
    : await import(`./dist/${name}.js`);

const entry = await load('entry');
const seg = await load('seg');
// `s_task` comes from the `task.js` entry chunk, and `s_foo` from the chunk of `dep.cjs`. They are
// the values that `entry.js` exports.
assert.strictEqual(seg.s_task, entry.task);
assert.strictEqual(seg.s_task(), 'task');
assert.strictEqual(seg.s_foo, entry.foo);
assert.strictEqual(seg.s_foo(), 'foo');
// The local exports of `seg.js` do not change.
assert.strictEqual(seg.v, seg.w);
assert.strictEqual(seg.v(), 'v');
assert.strictEqual(seg.x(), 'x');
