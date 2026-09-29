globalThis.dynamicImportImporterRuns ??= 0;
globalThis.dynamicImportImporterRuns++;

import('./target.js').then((mod) => {
  globalThis.dynamicImportImporterSawA = mod.a;
});

import.meta.hot.accept();
