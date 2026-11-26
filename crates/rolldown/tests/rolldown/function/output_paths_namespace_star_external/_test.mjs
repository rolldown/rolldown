import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

if (globalThis.__configName === 'cjs') {
  const require = createRequire(import.meta.url);
  assert.equal(require('./dist/main.cjs').hasFileURLToPath, false);
}
