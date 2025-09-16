import './dist/entry.js'

import assert from 'node:assert/strict';


setTimeout(() => {
  assert.deepEqual(globalThis.array, ['barundefined', 'fooundefined']);
}, 200)
