import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { assertNoStaticImportCycle } from '../../../_test_helpers/find-static-cycle.mjs';

// Static import cycles cause TDZ hazards or "X is not a function" runtime
// errors. Dynamic `import(...)` cycles cross async boundaries and are fine.
const distDir = path.join(import.meta.dirname, 'dist');
assertNoStaticImportCycle(distDir);

// Verify the dynamic-import chain resolves end-to-end and the re-exported
// value is correct while preserving dynamic1's observable namespace.
const dynamic1 = await import('./dist/dynamic1.js');
assert.ok(dynamic1.promise instanceof Promise, 'dynamic1 should export a promise');
const dynamic2 = await dynamic1.promise;
assert.strictEqual(dynamic2.sharedDynamic, true);

// Running the entry chunk must complete without throwing.
const distMain = path.join(distDir, 'main.js');
const stdout = execFileSync('node', [distMain], { encoding: 'utf-8' });
assert.strictEqual(stdout.trim(), 'true');
