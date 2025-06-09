import assert from 'node:assert'
import('./foo').then((foo) => {
  assert.strictEqual(foo.default, 'foo');
});

