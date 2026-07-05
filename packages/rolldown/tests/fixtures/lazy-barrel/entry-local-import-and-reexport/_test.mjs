import assert from 'node:assert';
import { useX } from './dist/barrel.js';

// Without the fix, lazy barrel drops barrel.js's `import { x }`, so the emitted
// `useX` references an unbound `x` and calling it throws `ReferenceError`.
assert.strictEqual(useX(), 'x-value');
