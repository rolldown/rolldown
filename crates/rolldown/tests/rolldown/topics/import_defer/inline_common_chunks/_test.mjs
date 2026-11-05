import assert from 'node:assert';

globalThis.log = [];
await import('./dist/a.js');
await import('./dist/b.js');
assert.deepStrictEqual(globalThis.log, []);
assert.strictEqual(globalThis.readA(), 'shared');
assert.strictEqual(globalThis.readB(), 'shared');
// The two files share one copy of `shared.js`, so it evaluates one time.
assert.deepStrictEqual(globalThis.log, ['shared']);
