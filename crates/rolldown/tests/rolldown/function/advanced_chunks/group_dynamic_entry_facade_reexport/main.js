import assert from 'node:assert';

import('./page.js')
  .then((page) => {
    assert.strictEqual(page.default(), 'header');
    return import('./layout.js');
  })
  .then((layout) => {
    assert.deepStrictEqual(Object.keys(layout), ['default', 'header']);
    assert.strictEqual(layout.default(), 'header');
  });
