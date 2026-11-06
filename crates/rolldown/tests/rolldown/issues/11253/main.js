import assert from 'node:assert';

// Each module id holds one of the four JS line terminators in its query.
const modules = await Promise.all([
  import('./odd.js?x\ny'),
  import('./odd.js?x\ry'),
  import('./odd.js?x\u2028y'),
  import('./odd.js?x\u2029y'),
]);

for (const module of modules) {
  assert.strictEqual(module.odd(), 'odd');
}
