import { b } from './target.js';

globalThis.nonAcceptedImporterRuns ??= 0;
globalThis.nonAcceptedImporterRuns++;
globalThis.nonAcceptedImporterSawB = b;

globalThis.nonAcceptedImporterAcceptCount ??= 0;
import.meta.hot.accept(() => {
  globalThis.nonAcceptedImporterAcceptCount++;
});
