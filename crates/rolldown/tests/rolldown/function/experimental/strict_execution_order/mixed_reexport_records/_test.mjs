import assert from 'node:assert/strict';

globalThis.events = [];
const { local, loadA, loadB } = await import('./dist/main.js');
assert.equal(local, 'required');
assert.deepEqual(globalThis.events, ['required', 'shared']);
const [a, again] = await Promise.all([loadA(), loadA()]);
assert.deepEqual(a, { value: 'a' });
assert.equal(a, again);
assert.deepEqual(await loadB(), { value: 'b' });
assert.deepEqual(globalThis.events, ['required', 'shared']);
