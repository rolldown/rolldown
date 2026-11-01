import assert from 'node:assert';

globalThis.nameVueScriptPart = [];
const { default: comp } = await import('./dist/main.js');

assert.strictEqual(comp, 1);
assert.deepStrictEqual(globalThis.nameVueScriptPart, ['script']);
