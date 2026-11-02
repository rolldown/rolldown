import assert from 'node:assert';
import { shared } from './shared.js';

let latest = shared();
assert.strictEqual(latest, 'shared+leaf-v0');
// The harness looks for a boundary only after a patch has registered its own rows, but the
// client looks before it fetches the patch, so a carrier must already have registered the
// inlined modules' rows.
assert.deepStrictEqual(__rolldown_runtime__.getImporters('leaf.js'), ['shared.js']);

import.meta.hot.accept('./shared.js', (mod) => {
  latest = mod.shared();
});

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(latest, 'shared+leaf-v1');
});
