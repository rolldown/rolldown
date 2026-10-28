import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const main =
  globalThis.__configName === 'cjs'
    ? createRequire(import.meta.url)('./dist/main.js')
    : await import('./dist/main.js');

if (globalThis.__configName !== 'extended-preserve-entry-signatures-allow-extension') {
  assert.deepEqual(Object.keys(main).sort(), ['POST']);
}
assert.deepEqual(await main.POST(), { same: true, value: 'shared:shared' });
assert.deepEqual(await main.POST(), { same: true, value: 'shared:shared' });
assert.equal(globalThis.sharedInitializations, 1);
