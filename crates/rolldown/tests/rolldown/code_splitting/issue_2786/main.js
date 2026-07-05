import shared from './share';
import sharedJson from './share.json';
import assert from 'node:assert/strict';

assert.strictEqual(shared, 'shared');
assert.deepStrictEqual(sharedJson, {});
console.log(shared, sharedJson);

import('./share').then((mod) => {
  assert.deepEqual(
    mod,
    Object.defineProperty(
      {
        default: 'shared',
      },
      Symbol.toStringTag,
      { value: 'Module' },
    ),
  );
  console.log(mod);
});
import('./share.json').then((mod) => {
  assert.deepEqual(
    mod,
    Object.defineProperty(
      {
        default: {},
      },
      Symbol.toStringTag,
      { value: 'Module' },
    ),
  );
  console.log(mod);
});
