import assert from 'node:assert';
import { foo } from './dist/main.js'
(async () => {
  const fooRes = await foo();
  assert.strictEqual(typeof fooRes.render, 'function');
  assert.strictEqual(await fooRes.render(), 'render');
})();

