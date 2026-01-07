import shared from './share';
import sharedJson from './share.json';
import assert from 'node:assert';

assert.strictEqual(shared, 'shared');
assert.deepStrictEqual(sharedJson, {});
console.log(shared, sharedJson);

import('./share').then((mod) => {
  // workaround for the String tag `Module`
  assert.deepEqual(JSON.parse(JSON.stringify(mod)), { default: 'shared' });
  console.log(mod);
});
import('./share.json').then((mod) => {
  // workaround for the String tag `Module`
  assert.deepEqual(JSON.parse(JSON.stringify(mod)), { default: {} });
  console.log(mod);
});
