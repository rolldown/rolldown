import assert from 'node:assert';

globalThis.log = [];
const { load, task } = await import('./dist/index.js');
assert.strictEqual(task(), 'task');
assert.deepStrictEqual(globalThis.log, ['task']);

// `import()` loads the facade of `seg_dynamic.js`. The facade runs only that module.
const namespace = await load();
assert.strictEqual(namespace.s_task(), 'task');
assert.deepStrictEqual(globalThis.log, ['task', 'seg_dynamic']);
