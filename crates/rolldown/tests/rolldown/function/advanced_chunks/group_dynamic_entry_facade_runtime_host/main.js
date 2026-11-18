import assert from 'node:assert';

import('./layout.js').then(async (layout) => {
  assert.deepStrictEqual(Object.keys(layout), ['default', 'loadPage']);
  assert.strictEqual(layout.default(), 'dep');
  const page = await layout.loadPage();
  assert.strictEqual(page.default(), 'p');
});
