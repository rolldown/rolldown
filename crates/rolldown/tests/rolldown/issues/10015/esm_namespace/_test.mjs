import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const main =
  globalThis.__configName === 'cjs'
    ? createRequire(import.meta.url)('./dist/main.js')
    : await import('./dist/main.js');

if (globalThis.__configName !== 'extended-preserve-entry-signatures-allow-extension') {
  assert.deepEqual(Object.keys(main).sort(), ['f', 'load']);
}
const { ns } = await main.load();
assert.deepEqual(Object.keys(ns).sort(), ['f', 'load']);
assert.equal(ns.f, main.f);
assert.equal(ns.load, main.load);
assert.equal(ns.f(), 42);
