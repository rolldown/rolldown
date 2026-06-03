import assert from 'node:assert';
import { modules } from './dist/main';

// The key should use the NFC form (precomposed),
// regardless of whether the glob pattern used NFD (decomposed).
const nfcKey = './\u30DD/a.js';

assert.ok(
  modules[nfcKey],
  `Expected glob to match NFC directory with NFD pattern. Keys: ${JSON.stringify(Object.keys(modules))}`,
);

modules[nfcKey]().then((m) => {
  assert.strictEqual(m.default, 'a');
});
