import path from 'node:path';
import { assertNoStaticImportCycle } from '../../_test_helpers/find-static-cycle.mjs';

// Regression for #11068: merging a common chunk into a dynamic entry chunk must not make
// that chunk import a chunk that already imports it.
assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

await import('./dist/main.js');
const [loadBarrel, loadPage] = globalThis.load;
if ((await loadBarrel()) !== 'layers') throw new Error('unexpected NAME');
if ((await loadPage()).TITLE !== 'LAYERS') throw new Error('unexpected TITLE');
