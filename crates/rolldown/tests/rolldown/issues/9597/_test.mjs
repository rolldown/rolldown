import path from 'node:path';
import { assertNoStaticImportCycle } from '../../_test_helpers/find-static-cycle.mjs';

// Regression for #9597: a cyclic import (entry-0 -> node3 -> node2 -> entry-0,
// plus dynamic import edges) placed the runtime-helper chunk inside the cycle,
// so executing `dist/entry-0.js` threw `TypeError: __exportAll is not a
// function` (the helper was read before its chunk finished initializing).
// Always-split-runtime-first keeps the runtime chunk out of the cycle.
assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

// Runtime smoke test for the original failure.
await import('./dist/entry-0.js');
