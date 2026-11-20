import assert from 'node:assert';

globalThis.__initLog = [];
const { read } = await import('./dist/main.js');
// In `cjs` output, `x` runs in the body of `b.js`, after `a`.
const expected = globalThis.__configName === 'cjs' ? ['a', 'x', 'b:1'] : ['x', 'a', 'b:1'];
assert.deepStrictEqual(globalThis.__initLog, expected);
assert.strictEqual(globalThis.__ySawA, true);
assert.strictEqual(read(), 'a');
delete globalThis.__initLog;
delete globalThis.__ySawA;
