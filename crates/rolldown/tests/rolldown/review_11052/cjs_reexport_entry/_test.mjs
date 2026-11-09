import assert from 'node:assert';
const m = await import('./dist/main.js');
assert.strictEqual(m.foo, 'cjs-foo');
assert.strictEqual(m.default, 'cjs-foo');
assert.strictEqual(m.bar(), 'local');
