import path from 'node:path';
import { assertNoCircularImports } from '../../../assert_no_circular_imports.mjs';

// Before the fix, external dynamic imports (@prettier/plugin-oxc, etc.)
// caused bit-position/chunk-index mismatches, producing a circular static
// import: babel.js <-> tt.js
//
// Expected: tt.js imports babel.js (not the reverse).
// babel.js should import from chunk.js (the runtime chunk), not tt.js.

assertNoCircularImports(path.join(import.meta.dirname, 'dist'));
