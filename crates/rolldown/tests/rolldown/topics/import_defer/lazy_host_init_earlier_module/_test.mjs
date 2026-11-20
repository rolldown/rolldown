import assert from 'node:assert';

globalThis.__initLog = [];
const { access } = await import('./dist/main.js');
assert.strictEqual(await access(), 'b,a,a');
assert.deepStrictEqual(globalThis.__initLog, ['x', 'a', 'y', 'b']);
delete globalThis.__initLog;
