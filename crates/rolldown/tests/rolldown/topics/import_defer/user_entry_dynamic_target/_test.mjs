import assert from 'node:assert';

globalThis.log = [];
const { loadDeferred } = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.log, ['main']);
// `import.defer()` loads the code of the entry `other.js`, but does not evaluate it.
const deferred = await loadDeferred();
assert.deepStrictEqual(globalThis.log, ['main']);
assert.strictEqual(deferred.value, 'other');
assert.deepStrictEqual(globalThis.log, ['main', 'other']);
const other = await import('./dist/other.js');
assert.strictEqual(other.value, 'other');
assert.deepStrictEqual(globalThis.log, ['main', 'other']);
