import path from 'node:path';
import { assertNoCircularImports } from '../../../assert_no_circular_imports.mjs';

// Before the fix, external dynamic imports (`@optional/ext`) caused
// bit-position/chunk-index mismatches in the chunk optimizer, producing
// a circular static import: parser-a.js <-> plugin.js
//
// Expected: plugin.js imports parser-a.js (not the reverse)

assertNoCircularImports(path.join(import.meta.dirname, 'dist'));
