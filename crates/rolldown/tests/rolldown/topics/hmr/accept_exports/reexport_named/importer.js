export { b } from './target.js';

globalThis.reexportNamedImporterRuns ??= 0;
globalThis.reexportNamedImporterRuns++;

globalThis.reexportNamedImporterAcceptCount ??= 0;
import.meta.hot.accept(() => {
  globalThis.reexportNamedImporterAcceptCount++;
});
