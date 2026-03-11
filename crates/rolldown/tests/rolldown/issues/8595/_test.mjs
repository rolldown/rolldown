import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Before the fix, external dynamic imports (@prettier/plugin-oxc, etc.)
// caused bit-position/chunk-index mismatches, producing a circular static
// import: babel.js <-> tt.js
//
// Expected: tt.js imports babel.js (not the reverse).
// babel.js should import from chunk.js (the runtime chunk), not tt.js.

const distDir = path.join(import.meta.dirname, 'dist');
const babel = fs.readFileSync(path.join(distDir, 'babel.js'), 'utf8');

assert(
  !babel.includes('from "./tt.js"'),
  'babel.js should not import from tt.js (would create a cycle)',
);
