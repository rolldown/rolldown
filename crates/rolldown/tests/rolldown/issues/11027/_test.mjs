import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const configName = globalThis.__configName;

if (configName.startsWith('esm')) {
  await import('./dist/main.js');
  const lib = await import('./dist/lib.js');
  assert.deepEqual(lib.default, { value: 42 });
  if (configName === 'esm') {
    // `exports-only`: the CommonJS entry keeps its `default`-only signature.
    assert.deepEqual(Object.keys(lib), ['default']);
  }
} else {
  const require = createRequire(import.meta.url);
  require('./dist/main.js');
  // The CommonJS entry's `module.exports` is exactly the source module's value.
  assert.deepEqual(require('./dist/lib.js'), { value: 42 });
}
assert.deepEqual(globalThis.mainResult, { value: 42, shared: 'shared' });
assert.equal(globalThis.sharedEvaluated, 1);
