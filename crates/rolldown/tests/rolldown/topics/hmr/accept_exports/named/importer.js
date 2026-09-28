import { a } from './target.js';

globalThis.namedImporterRuns ??= 0;
globalThis.namedImporterRuns++;
globalThis.namedImporterSawA = a;
