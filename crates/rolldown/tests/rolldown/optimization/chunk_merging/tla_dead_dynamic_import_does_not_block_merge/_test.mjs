import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertNoStaticImportCycle } from '../../../_test_helpers/find-static-cycle.mjs';

const distDir = path.join(import.meta.dirname, 'dist');
const files = fs
  .readdirSync(distDir)
  .filter((file) => file.endsWith('.js'))
  .sort();

assertNoStaticImportCycle(distDir);
assert.ok(
  !files.includes('main2.js'),
  `Dead dynamic imports in TLA modules must not block route-bit reduction for main/shared. Files: ${files.join(', ')}`,
);

assert.ok(
  !files.includes('shared.js'),
  `shared must fold into main.js, not be split into its own chunk. Files: ${files.join(', ')}`,
);
// main.js carries main's real code (with shared folded in), not a re-export facade.
// We assert on `const main` rather than a `//#region shared.js` marker because
// const-inlining folds `shared` straight into the `main:shared` literal and drops the marker.
const mainCode = fs.readFileSync(path.join(distDir, 'main.js'), 'utf8');
assert.ok(mainCode.includes('const main'), 'main.js should hold main + shared, not be a facade');

const loader = await import(pathToFileURL(path.join(distDir, 'loader.js')).href);
assert.strictEqual(loader.loaded.route, 'main:shared:route');
