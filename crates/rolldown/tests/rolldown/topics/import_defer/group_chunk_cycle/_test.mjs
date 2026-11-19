import assert from 'node:assert';

globalThis.__hoistLog = [];
await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__hoistLog, ['warm', 'y', 'a:xy', 'b:1']);
delete globalThis.__hoistLog;
