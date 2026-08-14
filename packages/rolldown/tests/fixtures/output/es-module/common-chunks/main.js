import assert from 'node:assert';

import('./shared').then((imported) => {
  assert.strictEqual(imported.shared, 'shared');
});

export const main = 'main';
