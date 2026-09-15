import assert from 'node:assert';

const { targetPromise } = await import('./dist/a.js');
const target = await targetPromise;

assert.strictEqual(target.value, 1);

// Link the consumer chunk too: `b.js` imports the renamed binding from the host chunk, so a
// producer/consumer disagreement on the new name would surface here as a SyntaxError.
await import('./dist/b.js');
assert.strictEqual(typeof globalThis.hostThen, 'function');
