export { n, reads } from './counter.cjs';

globalThis.bRuns = (globalThis.bRuns ?? 0) + 1;
