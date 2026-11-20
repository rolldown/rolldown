import assert from 'node:assert';

globalThis.__initLog = [];
const { read } = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__initLog, ['a', 'y', 'b']);
assert.strictEqual(read(), 'a');
delete globalThis.__initLog;
