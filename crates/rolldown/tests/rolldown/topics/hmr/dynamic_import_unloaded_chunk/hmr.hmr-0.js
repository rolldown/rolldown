import assert from 'node:assert/strict';

export const value = 'v2';

// The edit adds an `import()` of route.js, which this client never loaded.
import('./route.js').then((mod) => {
  assert.strictEqual(mod.value, 'route:shared:dep');
  globalThis.__dynamic_import_unloaded_chunk = true;
});
process.on('beforeExit', () => assert.ok(globalThis.__dynamic_import_unloaded_chunk));

import.meta.hot.accept();
