import assert from 'node:assert';

globalThis.__extLog = [];
const main = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__extLog, ['main']);

// The first access evaluates `d.js`.
assert.strictEqual(main.read(), 1);
assert.deepStrictEqual(globalThis.__extLog, ['main', 'd']);

delete globalThis.__extLog;
