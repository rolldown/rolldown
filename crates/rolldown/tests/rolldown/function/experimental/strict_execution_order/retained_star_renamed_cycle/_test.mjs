import assert from 'node:assert/strict';

await import('./dist/main.js');

// The importer-local routing repair removes the projected page-b -> page-a edge that previously
// forced page-a through a collapsed facade. Its direct namespace must still preserve the narrowed
// dynamic-import interface without retaining renamed aliases or dangling getters.
const pageANamespace = await import('./dist/page-a.js');
assert.doesNotThrow(
  () => ({ ...pageANamespace }),
  'every namespace getter should resolve to a live binding',
);
assert.deepStrictEqual(
  Object.keys(pageANamespace),
  ['_', 'common'],
  "the namespace should preserve the dynamic import's narrowed export interface",
);
assert.strictEqual(pageANamespace.common, 'common');
