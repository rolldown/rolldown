import value from './target.js';

globalThis.defaultImportImporterRuns ??= 0;
globalThis.defaultImportImporterRuns++;
globalThis.defaultImportImporterSaw = value;
