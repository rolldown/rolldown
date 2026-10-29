import assert from 'node:assert';

const { app } = await import('./dist/main.js');
const appNs = await app;

// `import()` of a CommonJS module resolves to its interop namespace: the
// `module.exports` object is the `default` export.
assert.strictEqual(appNs.default.own, 43);
assert.strictEqual(await appNs.default.done, 84);
