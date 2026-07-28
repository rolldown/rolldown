import assert from 'node:assert/strict';

await import('./dist/main.js');

// Reading `pageA.x` in the source fixture would make `x` part of DynamicImportExportsUsage and
// stop testing the narrowed interface. Inspect the namespace published by the implementation chunk
// instead; its export key may be minified, so identify the only object export by shape.
const implementation = await import('./dist/page-a.js');
const pageANamespace = Object.values(implementation).find(
  (value) => value && typeof value === 'object' && 'common' in value,
);

assert.ok(pageANamespace, 'the collapsed dynamic entry should publish its simulated namespace');
assert.doesNotThrow(
  () => ({ ...pageANamespace }),
  'every simulated namespace getter should resolve to a live binding',
);
assert.deepStrictEqual(
  Object.keys(pageANamespace),
  ['_', 'common'],
  "the simulated namespace should preserve the original facade's narrowed export interface",
);
