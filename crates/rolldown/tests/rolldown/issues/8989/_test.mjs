import path from 'node:path';
import { assertNoStaticImportCycle } from '../../_test_helpers/find-static-cycle.mjs';

// Regression for #8989: facade chunk elimination used to add a runtime-helper
// edge from entry2 → the chunk hosting the runtime, while that host chunk
// already had a forward path entry2 → entry3 → node4 → entry2, closing a cycle.
// The exact chunk shape may change as long as generated entry chunks do not
// contain circular static references.
assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

// Runtime smoke test — every entry chunk must load without throwing.
for (const entry of ['entry0.js', 'entry1.js', 'entry2.js', 'entry3.js']) {
  await import(`./dist/${entry}`);
}
