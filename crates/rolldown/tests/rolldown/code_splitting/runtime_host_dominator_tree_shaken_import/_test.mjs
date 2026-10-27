import assert from 'node:assert/strict';

// `c.js` and `logger.js` import `consumer.js` only for a binding that tree shaking removes. The
// dominator search must not follow these imports. If it did, `consumer.js` would host the runtime,
// and entry `a` would load it through `logger.js`, with its external import `server-only`.
const { a } = await import('./dist/a.js');
assert.equal(a, 'log: a');
assert.equal(globalThis.__consumer_ran, undefined);

const { keys } = await import('./dist/c.js');
assert.deepEqual(keys, ['a', 'b']);
assert.equal(globalThis.__consumer_ran, undefined);
