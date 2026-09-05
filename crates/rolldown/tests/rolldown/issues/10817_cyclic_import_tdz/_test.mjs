import assert from 'node:assert/strict';

await assert.rejects(
  import('./dist/main.js'),
  (error) => error instanceof ReferenceError && String(error).includes('VALUE'),
  'the cyclic import must read VALUE while it is in the TDZ',
);
