import assert from 'node:assert/strict';

const events = [];
globalThis.__event = (name) => events.push(name);
await import('./dist/main.js');
await globalThis.run();
// `route.mjs` imports `leaf.cjs` before `cycle.mjs`, so `leaf.cjs` runs first. The chunk of
// `route.mjs` must keep that order after a signature facade splits it.
assert.deepEqual(events, ['leaf', 'cycle']);
