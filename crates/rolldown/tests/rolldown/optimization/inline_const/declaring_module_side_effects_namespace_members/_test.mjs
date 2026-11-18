import assert from 'node:assert/strict';

// `z` reads `ui.B` before `ui.A`. The chunks of `b.js` and `a.js` take the position of the `ui`
// import in their `exec_order`, in which `a.js` comes first.
await import('./dist/z.js');
assert.deepEqual(globalThis.log, ['a', 'b']);
