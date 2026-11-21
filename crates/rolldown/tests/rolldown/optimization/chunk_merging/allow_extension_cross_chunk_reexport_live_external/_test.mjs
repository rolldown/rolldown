import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

// Code outside the bundle loads the facade of `seg.js`.
const seg = globalThis.__configName?.startsWith('cjs')
  ? require('./dist/seg.js')
  : await import('./dist/seg.js');

assert.strictEqual(seg.foreign, 5);
assert.strictEqual(seg.value, 2);
seg.change();
// `value` is a live binding of `ext`. Only `externalLiveBindings: false` gives a copy.
assert.strictEqual(seg.value, globalThis.__configName === 'cjs-no-live-bindings' ? 2 : 3);
