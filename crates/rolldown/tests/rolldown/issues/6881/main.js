import assert from 'node:assert/strict';
const testJson = await import('./test.json').then((r) => {
  assert.deepEqual(r.default, { hello: 'Hola' });
});

export { testJson };
