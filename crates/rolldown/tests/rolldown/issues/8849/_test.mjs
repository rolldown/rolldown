import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Before the fix, the chunk optimizer merged value.js into the admin entry
// chunk, causing common.js (a dynamic import) to import from admin.js.
// This means loading main.js would also execute admin.js as a side effect.
//
// Expected: common.js should NOT import from admin.js.

const distDir = path.join(import.meta.dirname, 'dist');
const common = fs.readFileSync(path.join(distDir, 'common.js'), 'utf8');

assert(
  !common.includes('from "./admin.js"'),
  'common.js should not import from admin.js (would cause admin side effects when loading main)',
);
