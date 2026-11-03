import assert from 'node:assert/strict';

await assert.rejects(
  import('./dist/b.js'),
  (error) => error instanceof ReferenceError && String(error).includes('VALUE'),
  'loading the second entry must preserve the cyclic TDZ failure',
);

await assert.rejects(
  import('./dist/call-b.js'),
  (error) => error instanceof ReferenceError && String(error).includes('VALUE'),
  'the analyzer rerun for an imported pure function must preserve the cyclic TDZ failure',
);
