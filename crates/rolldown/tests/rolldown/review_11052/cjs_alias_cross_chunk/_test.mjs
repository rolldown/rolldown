import assert from 'node:assert';
const { a } = await import('./dist/a.js');
const { b } = await import('./dist/b.js');
assert.deepStrictEqual(a(), ['cjs-foo', 'local']);
assert.deepStrictEqual(b(), ['cjs-foo', 'local']);
