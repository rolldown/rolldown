import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Pre-fix, the renamed `require_main` landed on its sibling's name and the ESM bundle failed to
// parse with `SyntaxError: Identifier 'require_main$1' has already been declared`.
const require = createRequire(import.meta.url);
const mod =
  globalThis.__configName === 'cjs'
    ? require('./dist/main.js')
    : (await import('./dist/main.js')).default;

assert.deepEqual(
  mod.pair.map((item) => item.value),
  ['a', 'b'],
);
