import assert from 'node:assert';

let unhandled = 0;
process.on('unhandledRejection', () => unhandled++);
const { first, second, log } = await import('./dist/main.js');
await new Promise((resolve) => setTimeout(resolve, 10));

assert.deepStrictEqual(log, ['a']);
assert.strictEqual(first, 'm failed');
assert.strictEqual(second, 'm failed');
assert.strictEqual(unhandled, 0);
