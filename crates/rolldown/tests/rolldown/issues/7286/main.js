import assert from 'node:assert';
const A = { A: 'A' };

const { [A.A]: foo } = { A: 'foo' };

assert.strictEqual(foo, 'foo');
