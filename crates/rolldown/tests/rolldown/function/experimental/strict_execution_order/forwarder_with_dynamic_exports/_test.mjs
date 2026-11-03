import assert from 'node:assert/strict';
import path from 'node:path';
import { assertNoStaticImportCycle } from '../../../../_test_helpers/find-static-cycle.mjs';

assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

await import('./dist/page.js');
await import('./dist/app.js');
assert.equal(globalThis.__page, false);
assert.deepEqual(globalThis.__app, [true, true, true, true]);
