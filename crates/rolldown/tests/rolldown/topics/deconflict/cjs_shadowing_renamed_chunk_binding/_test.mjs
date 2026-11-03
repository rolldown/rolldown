import { strict as assert } from 'node:assert';

// Regression test for a CJS closure shadowing a chunk-root binding after that binding is
// renamed by normal root-scope deconfliction.
await import('./dist/main.js');

// Reaching this line means the wrapped entry executed without throwing.
assert.ok(true);
