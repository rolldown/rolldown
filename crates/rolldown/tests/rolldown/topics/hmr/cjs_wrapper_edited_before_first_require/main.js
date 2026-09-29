import assert from 'node:assert';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(require('./cfg.cjs').value, 'v2');
});
