import assert from 'node:assert/strict';
const m = await import('./dist/entry.js');
assert.equal(m.value, 'ok');
const exp = await m.default.fetch();
assert.deepEqual(await exp.default(), ['A:router(ctx)', 'router(ctx)']);
