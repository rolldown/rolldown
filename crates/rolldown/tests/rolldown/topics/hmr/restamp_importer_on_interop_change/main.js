import assert from 'node:assert';
import './importer.js';
import './watcher.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.deepStrictEqual(globalThis.__restamp_importer_value, { v: 2 });
});
