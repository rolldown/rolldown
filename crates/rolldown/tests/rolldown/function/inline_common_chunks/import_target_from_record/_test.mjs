import assert from 'node:assert';

// `loader.js` is inlined into `a` and `b`; both copies of its `import()` reach one `m.js`.
globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
const fromA = await globalThis.loadFromA();
await import('./dist/x.js');
const fromB = await globalThis.loadFromB();

assert.deepStrictEqual(globalThis.events, ['M body', 'X 1']);
assert.strictEqual(fromA, fromB);
assert.strictEqual(fromA.count, 1);
