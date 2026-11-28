import assert from 'node:assert';

globalThis.__initLog = [];
const { access } = await import('./dist/main.js');
assert.strictEqual(await access(), 'p,q,a,a');
assert.deepStrictEqual(globalThis.__initLog, ['a', 'y', 'p', 'q']);
delete globalThis.__initLog;
