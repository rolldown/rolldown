import assert from 'node:assert';

globalThis.__initLog = [];
const { read } = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__initLog, ['wait', 'a', 'b']);
assert.strictEqual(read(), 'a');
assert.strictEqual(globalThis.__yRan, true);
delete globalThis.__initLog;
delete globalThis.__yRan;
