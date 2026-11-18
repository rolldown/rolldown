import { a } from './target.js';
import { b } from './target.js';

globalThis.multiRecordImporterRuns ??= 0;
globalThis.multiRecordImporterRuns++;
globalThis.multiRecordImporterSaw = `${a}${b}`;

globalThis.multiRecordImporterAcceptCount ??= 0;
import.meta.hot.accept(() => {
  globalThis.multiRecordImporterAcceptCount++;
});
