import assert from 'node:assert';
import { value } from './dist/main.js';

// deep.js sets __deepReady after four awaits.  Even in the mixed
// barrel+direct case where init_deep may be awaited twice (once
// transitively via init_barrel, once directly), the init must be fully
// completed before any consumer code runs.
assert.strictEqual(value, 'deep-value');
assert.strictEqual(
  globalThis.__deepReady,
  true,
  'deep.js TLA not fully awaited in mixed barrel+direct case',
);
