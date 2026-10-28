export * from './target.js';

globalThis.reexportStarImporterRuns ??= 0;
globalThis.reexportStarImporterRuns++;

globalThis.reexportStarImporterAcceptCount ??= 0;
import.meta.hot.accept(() => {
  globalThis.reexportStarImporterAcceptCount++;
});
