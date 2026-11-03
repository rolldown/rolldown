import assert from 'node:assert';

globalThis.__events = [];
const { read } = await import('./dist/main.js');

assert.deepStrictEqual(read(), {
  defaultValue: 'default',
  aliasDefault: 'default',
  value: 'named',
  alias: 'named',
  sameDefault: true,
  sameNamespace: true,
});
// Repeated imports must evaluate dep only once, before the other module request.
assert.deepStrictEqual(globalThis.__events, ['dep', 'other']);
