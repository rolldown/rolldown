import assert from 'node:assert';
import './named/importer';
import './non_accepted/importer';
import './promoted/importer';
import './side_effect/importer';
import './default_import/importer';
import './dynamic_import/importer';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(globalThis.namedAcceptCount, 1);
  assert.strictEqual(globalThis.nonAcceptedAcceptCount, 1);
  assert.strictEqual(globalThis.promotedAcceptCount, 1);
  assert.strictEqual(globalThis.sideEffectAcceptCount, 1);
  assert.strictEqual(globalThis.defaultImportAcceptCount, 1);
  assert.strictEqual(globalThis.dynamicImportAcceptCount, 1);

  assert.strictEqual(globalThis.namedImporterRuns, 1);
  assert.strictEqual(globalThis.promotedImporterRuns, 1);
  assert.strictEqual(globalThis.sideEffectImporterRuns, 1);
  assert.strictEqual(globalThis.defaultImportImporterRuns, 1);
  assert.strictEqual(globalThis.namedImporterSawA, 0);
  assert.strictEqual(globalThis.defaultImportImporterSaw, 0);

  assert.strictEqual(globalThis.nonAcceptedImporterRuns, 2);
  assert.strictEqual(globalThis.nonAcceptedImporterSawB, 'b2');
  assert.strictEqual(globalThis.nonAcceptedImporterAcceptCount, 1);
  assert.strictEqual(globalThis.dynamicImportImporterRuns, 2);
  assert.strictEqual(globalThis.dynamicImportImporterSawA, 1);
});
