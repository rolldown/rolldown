import assert from 'node:assert';

globalThis.nameVueScriptPartEntry = [];
const { default: comp } = await import('./dist/Comp.vue.js');

assert.strictEqual(comp, 1);
assert.deepStrictEqual(globalThis.nameVueScriptPartEntry, ['script', 'wrapper']);
