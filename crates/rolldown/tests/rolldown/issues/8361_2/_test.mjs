import path from 'node:path';
import { assertNoStaticImportCycle } from '../../_test_helpers/find-static-cycle.mjs';

// The bug was `__commonJSMin is not a function` due to circular static imports
// caused by chunk optimization merging the runtime into the entry chunk.
// The exact chunk shape may change as long as generated entry chunks do not
// contain circular static references.
assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

// The original bug surfaced as `__commonJSMin is not a function` at runtime,
// so importing the entry must not throw.
await import('./dist/main.js');
