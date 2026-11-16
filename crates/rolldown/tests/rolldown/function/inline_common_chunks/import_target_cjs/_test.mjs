import assert from 'node:assert';

// A CommonJS module loaded with `import()` and imported statically keeps one `module.exports`.
globalThis.events = [];
await import('./dist/x.js');
await import('./dist/y.js');
const ns = await globalThis.loadCounter();
ns.default.inc();

assert.deepStrictEqual(globalThis.events, ['counter body', 'X 1']);
assert.strictEqual(ns.default.count, 2);
