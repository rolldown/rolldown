import assert from 'node:assert';
import path from 'node:path';
import { assertNoStaticImportCycle } from '../../../../_test_helpers/find-static-cycle.mjs';

assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

// Runtime check: importing the entry must run node1 → init_node2 → (async) node0
// without throwing, and every fuzz counter must end up assigned.
await import('./dist/node1.js');
await new Promise((resolve) => setImmediate(resolve));
for (const idx of [0, 1, 2]) {
  assert.strictEqual(
    globalThis[`__acyclic_output_fuzz_${idx}`],
    idx,
    `node${idx} body must have executed and set its fuzz counter`,
  );
}
