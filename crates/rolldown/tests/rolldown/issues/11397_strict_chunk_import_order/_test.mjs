import assert from 'node:assert/strict';

const events = [];
globalThis.__event = (name) => events.push(name);
await import('./dist/main.js');
await globalThis.run();
// `page.mjs` imports `leaf1.mjs` before `leaf0.mjs`. The chunk of `route2.mjs` holds `leaf0.mjs`,
// so `page.js` must still import it after the chunk of `leaf1.mjs` when a signature facade splits
// the chunk of `route2.mjs`.
assert.deepEqual(events, ['leaf1', 'leaf0']);
