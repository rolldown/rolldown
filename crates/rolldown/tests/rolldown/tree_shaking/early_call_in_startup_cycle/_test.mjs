import assert from 'node:assert/strict';

process.setSourceMapsEnabled(true);
await assert.rejects(import('./dist/main.js'), (error) => {
  assert.ok(error instanceof ReferenceError);
  assert.match(error.stack.split('\n')[1], /[/\\]caller\.js:2:1\)?$/);
  return true;
});
