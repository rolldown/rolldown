import assert from 'node:assert';
import { current } from './accept_dep.js';

assert.strictEqual(current(), 'dep-v0');

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(current(), 'dep-v1');
});
