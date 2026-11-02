import assert from 'node:assert/strict';

// `consumer.js` is an entry without side effects. `c.js` and `logger.js` import it only for a
// binding that tree shaking removes, so the output does not import its chunk from them. The
// dominator search must not follow these imports, or entry `a` loads `consumer.js` through the
// helper import of `logger.js`, with its external import `server-only`.
const { a } = await import('./dist/a.js');
assert.equal(a, 'log: a');
assert.equal(globalThis.__consumer_ran, undefined);

const { keys } = await import('./dist/c.js');
assert.deepEqual(keys, ['a', 'b']);
assert.equal(globalThis.__consumer_ran, undefined);
