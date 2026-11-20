import assert from 'node:assert';

globalThis.__initLog = [];
const { access } = await import('./dist/main.js');
// `y.js` reads the log when it runs, so `a.js` must run before it.
assert.deepStrictEqual(await access(), [true, 'a']);
assert.deepStrictEqual(globalThis.__initLog, ['a', 'y']);
delete globalThis.__initLog;
