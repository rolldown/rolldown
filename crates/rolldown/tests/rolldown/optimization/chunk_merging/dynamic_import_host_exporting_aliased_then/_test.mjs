import assert from 'node:assert';

const { targetPromise, target2Promise } = await import('./dist/a.js');
const target = await targetPromise;

assert.strictEqual(target.value, 1);
assert.strictEqual((await target2Promise).value2, 2);

// Link the consumer chunk too: `b.js` imports the renamed binding from the host chunk, so a
// producer/consumer disagreement on the new name would surface here as a SyntaxError.
await import('./dist/b.js');
assert.strictEqual(typeof globalThis.hostThen, 'function');
