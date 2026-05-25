import path from 'node:path';
import { assertNoStaticImportCycle } from '../../_test_helpers/find-static-cycle.mjs';

// Regression for #9401: avoidRedundantChunkLoads must not reduce the
// runtime-helper host into entry-0 when node3 also needs helpers and entry-0
// already statically reaches node3 through a CJS require chain.
assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

// Runtime smoke test for the original failure:
// TypeError: __exportAll is not a function.
await import('./dist/entry-0.js');
