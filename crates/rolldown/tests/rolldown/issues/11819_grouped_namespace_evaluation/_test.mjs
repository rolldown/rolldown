import assert from 'node:assert/strict';

Object.prototype.reproKey = 1;
try {
  await assert.rejects(import('./dist/main.js'), { message: 'waiter stopped' });
} finally {
  delete Object.prototype.reproKey;
}
