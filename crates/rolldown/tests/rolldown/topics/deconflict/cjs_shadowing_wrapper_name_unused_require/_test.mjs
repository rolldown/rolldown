import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Companion to `cjs_shadowing_suffixed_wrapper_name`. Here `./b/dup.cjs` is required for its side
// effect alone, which sets `ImportRecordMeta::IsRequireUnused` on the record. The finalizer still
// emits `require_dup$1()`, so the author-local `require_dup$1` must still be renamed. Otherwise
// that call resolves to the local string and the bundle throws
// `TypeError: require_dup$1 is not a function`.
const require = createRequire(import.meta.url);

globalThis.__dupBEvaluated = false;

const mod =
  globalThis.__configName === 'cjs'
    ? require('./dist/main.js')
    : (await import('./dist/main.js')).default;

assert.deepEqual(mod, { a: 'a', local: 'local-string' });
assert.equal(globalThis.__dupBEvaluated, true);
