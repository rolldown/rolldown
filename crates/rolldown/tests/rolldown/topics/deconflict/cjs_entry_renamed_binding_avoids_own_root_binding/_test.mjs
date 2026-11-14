import { strict as assert } from 'node:assert';

// Before the fix, the renamed `Promise` took the name `Promise$2`, and the bundle failed to parse
// with `SyntaxError: Identifier 'Promise$2' has already been declared`.
const main = (await import('./dist/main.js')).default;
assert.deepEqual(main, ['function', 'sibling', 'nested']);
