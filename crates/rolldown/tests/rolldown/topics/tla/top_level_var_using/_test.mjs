import assert from 'node:assert/strict';

if (Number(process.versions.node.split('.')[0]) >= 24) {
  const { log } = await import('./dist/main.js');

  assert.deepEqual(log, ['body', 'object', 'object', 'async disposed', 'sync disposed']);
}
