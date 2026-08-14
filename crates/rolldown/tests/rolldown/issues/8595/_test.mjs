import path from 'node:path';
import { assertNoStaticImportCycle } from '../../_test_helpers/find-static-cycle.mjs';

// Original bug (#8595): external dynamic imports caused bit-position/chunk-index
// mismatches that produced a circular static import babel.js <-> tt.js.
// Static import cycles cause TDZ hazards, so verify the static graph is acyclic.
assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));
