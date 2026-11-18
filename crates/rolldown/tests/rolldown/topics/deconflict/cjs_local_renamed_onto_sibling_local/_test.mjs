import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Pre-fix, the renamed `require_dup` landed on its sibling's name and the bundle failed to parse
// with `SyntaxError: Identifier 'require_dup$1' has already been declared`.
const require = createRequire(import.meta.url);
const mod =
  globalThis.__configName === 'cjs'
    ? require('./dist/main.js')
    : (await import('./dist/main.js')).default;

assert.deepEqual(mod, ['a', 'b']);
