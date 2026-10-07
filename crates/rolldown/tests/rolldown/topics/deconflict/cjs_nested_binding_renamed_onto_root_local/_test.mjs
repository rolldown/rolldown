import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Pre-fix, the renamed parameter captured the root-scope local and this returned
// `['param', 'param']` with no error at all.
const require = createRequire(import.meta.url);
const result =
  globalThis.__configName === 'cjs'
    ? require('./dist/main.js')
    : (await import('./dist/main.js')).default;

assert.deepEqual(result, ['param', 'root-local']);
