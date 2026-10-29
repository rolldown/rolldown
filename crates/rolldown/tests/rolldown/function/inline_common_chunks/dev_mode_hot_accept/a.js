import assert from 'node:assert';
import { value } from './self_accept.js';

assert.strictEqual(value, 'self-v0');

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(globalThis.selfAccepted, 'self-v1');
});
