import assert from 'node:assert';

import('./page.js')
  .then((page) => {
    assert.strictEqual(page.default(), 'header');
    assert.deepStrictEqual(globalThis.log, ['header', 'page']);
    return import('./layout.js');
  })
  .then((layout) => {
    assert.strictEqual(layout.default(), 'header');
  });
