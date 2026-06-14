import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertNoStaticImportCycle } from '../../../_test_helpers/find-static-cycle.mjs';

const distDir = path.join(import.meta.dirname, 'dist');
const files = fs.readdirSync(distDir).filter((file) => file.endsWith('.js')).sort();

assertNoStaticImportCycle(distDir);
assert.ok(
  !files.includes('main2.js'),
  `Dead dynamic imports in TLA modules must not block route-bit reduction for main/shared. Files: ${files.join(', ')}`,
);

const mainCode = fs.readFileSync(path.join(distDir, 'main.js'), 'utf8');
assert.ok(mainCode.includes('//#region shared.js'), 'shared.js should stay with main.js');

const loader = await import(pathToFileURL(path.join(distDir, 'loader.js')).href);
assert.strictEqual(loader.loaded.route, 'main:shared:route');
