import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Before the fix, external dynamic imports (`@optional/ext`) caused
// bit-position/chunk-index mismatches in the chunk optimizer, producing
// a circular static import: parser-a.js <-> plugin.js
//
// Expected: plugin.js imports parser-a.js (not the reverse)

const distDir = path.join(import.meta.dirname, 'dist');
const parserA = fs.readFileSync(path.join(distDir, 'parser-a.js'), 'utf8');

assert(
  !parserA.includes('from "./plugin.js"'),
  'parser-a.js should not import from plugin.js (would create a cycle)',
);
