import assert from 'node:assert/strict';

if (Number(process.versions.node.split('.')[0]) >= 24) {
  const { log } = await import('./dist/main.js');

  assert.deepEqual(log, ['body', 'object', 'object', 'async disposed', 'sync disposed']);

  const referenced = await import('./dist/referenced.js');

  assert.deepEqual(referenced.log, ['async disposed', 'sync disposed']);
  assert.equal(referenced.getSync().kind, 'sync');
  assert.equal(referenced.getAsync().kind, 'async');
}
