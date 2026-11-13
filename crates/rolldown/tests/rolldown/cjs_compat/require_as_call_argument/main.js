import assert from 'node:assert';

function foo(id, req) {
  return [id, typeof req];
}

assert.deepStrictEqual(foo('./missing.js', require), ['./missing.js', 'function']);
